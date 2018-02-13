// Copyright 2016 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement.  This, along with the Licenses can be
// found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

#[cfg(feature = "use-mock-crust")]
use fake_clock::FakeClock as Instant;
use itertools::Itertools;
use maidsafe_utilities::serialisation;
use messages::SignedMessage;
use public_info::PublicInfo;
use rust_sodium::crypto::sign;
use sha3::Digest256;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
#[cfg(not(feature = "use-mock-crust"))]
use std::time::Instant;
use tiny_keccak::sha3_256;

/// Time (in seconds) within which a message and a quorum of signatures need to arrive to
/// accumulate.
pub const ACCUMULATION_TIMEOUT_SECS: u64 = 30;

#[derive(Default)]
pub struct SignatureAccumulator {
    sigs: HashMap<Digest256, (Vec<(PublicInfo, sign::Signature)>, Instant)>,
    msgs: HashMap<Digest256, (SignedMessage, u8, Instant)>,
}

impl SignatureAccumulator {
    /// Adds the given signature to the list of pending signatures or to the appropriate
    /// `SignedMessage`. Returns the message, if it has enough signatures now.
    pub fn add_signature(
        &mut self,
        group_size: usize,
        hash: Digest256,
        sig: sign::Signature,
        pub_info: PublicInfo,
    ) -> Option<(SignedMessage, u8)> {
        self.remove_expired();
        if let Some(&mut (ref mut msg, _, _)) = self.msgs.get_mut(&hash) {
            msg.add_signature(pub_info, sig);
        } else {
            let sigs_vec = self.sigs.entry(hash).or_insert_with(
                || (vec![], Instant::now()),
            );
            sigs_vec.0.push((pub_info, sig));
            return None;
        }
        self.remove_if_complete(group_size, &hash)
    }

    /// Adds the given message to the list of pending messages. Returns it if it has enough
    /// signatures.
    pub fn add_message(
        &mut self,
        mut msg: SignedMessage,
        group_size: usize,
        route: u8,
    ) -> Option<(SignedMessage, u8)> {
        self.remove_expired();
        let hash = match serialisation::serialise(msg.routing_message()) {
            Ok(serialised_msg) => sha3_256(&serialised_msg),
            Err(err) => {
                error!("Failed to serialise {:?}: {:?}.", msg, err);
                return None;
            }
        };
        match self.msgs.entry(hash) {
            Entry::Occupied(mut entry) => {
                // TODO - should update `route` of `entry`?
                trace!("Received two full SignedMessages {:?}.", msg);
                entry.get_mut().0.add_signatures(msg);
            }
            Entry::Vacant(entry) => {
                for (pub_info, sig) in self.sigs.remove(&hash).into_iter().flat_map(
                    |(vec, _)| vec,
                )
                {
                    msg.add_signature(pub_info, sig);
                }
                let _ = entry.insert((msg, route, Instant::now()));
            }
        }
        self.remove_if_complete(group_size, &hash)
    }

    fn remove_expired(&mut self) {
        let expired_sigs = self.sigs
            .iter()
            .filter(|&(_, &(_, ref time))| {
                time.elapsed().as_secs() > ACCUMULATION_TIMEOUT_SECS
            })
            .map(|(hash, _)| *hash)
            .collect_vec();
        for hash in expired_sigs {
            let _ = self.sigs.remove(&hash);
        }
        let expired_msgs = self.msgs
            .iter()
            .filter(|&(_, &(_, _, ref time))| {
                time.elapsed().as_secs() > ACCUMULATION_TIMEOUT_SECS
            })
            .map(|(hash, _)| *hash)
            .collect_vec();
        for hash in expired_msgs {
            let _ = self.msgs.remove(&hash);
        }
    }

    fn remove_if_complete(
        &mut self,
        group_size: usize,
        hash: &Digest256,
    ) -> Option<(SignedMessage, u8)> {
        match self.msgs.get_mut(hash) {
            None => return None,
            Some(&mut (ref mut msg, _, _)) => {
                if !msg.check_fully_signed(group_size) {
                    return None;
                }
            }
        }
        self.msgs.remove(hash).map(|(msg, route, _)| (msg, route))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use full_info::FullInfo;
    use itertools::Itertools;
    use messages::{DirectMessage, MessageContent, RoutingMessage, SectionList, SignedMessage};
    use public_info::PublicInfo;
    use rand;
    use routing_table::Authority;
    use routing_table::Prefix;
    use std::collections::BTreeSet;

    struct MessageAndSignatures {
        signed_msg: SignedMessage,
        signature_msgs: Vec<DirectMessage>,
    }

    impl MessageAndSignatures {
        fn new<'a, I>(
            msg_sender_info: &FullInfo,
            other_infos: I,
            all_infos: BTreeSet<PublicInfo>,
        ) -> MessageAndSignatures
        where
            I: Iterator<Item = &'a FullInfo>,
        {
            let routing_msg = RoutingMessage {
                src: Authority::ClientManager(rand::random()),
                dst: Authority::ClientManager(rand::random()),
                content: MessageContent::SectionSplit(
                    Prefix::new(0, rand::random()).with_version(0),
                    rand::random(),
                ),
            };
            let prefix = Prefix::new(0, unwrap!(all_infos.iter().next()).name());
            let lists = vec![SectionList::new(prefix, all_infos)];
            let signed_msg = unwrap!(SignedMessage::new(routing_msg, msg_sender_info, lists));
            let signature_msgs = other_infos
                .map(|id| {
                    unwrap!(signed_msg.routing_message().to_signature(
                        id.secret_sign_key(),
                    ))
                })
                .collect();
            MessageAndSignatures {
                signed_msg: signed_msg,
                signature_msgs: signature_msgs,
            }
        }
    }

    struct Env {
        _msg_sender_info: FullInfo,
        other_infos: Vec<FullInfo>,
        senders: BTreeSet<PublicInfo>,
        msgs_and_sigs: Vec<MessageAndSignatures>,
    }

    impl Env {
        fn new() -> Env {
            let msg_sender_info = FullInfo::node_new(1u8);
            let mut pub_infos = vec![*msg_sender_info.public_info()]
                .into_iter()
                .collect::<BTreeSet<_>>();
            let mut other_infos = vec![];
            for _ in 0..8 {
                let full_info = FullInfo::node_new(1u8);
                let _ = pub_infos.insert(*full_info.public_info());
                other_infos.push(full_info);
            }
            let msgs_and_sigs = (0..5)
                .map(|_| {
                    MessageAndSignatures::new(
                        &msg_sender_info,
                        other_infos.iter(),
                        pub_infos.clone(),
                    )
                })
                .collect();
            Env {
                _msg_sender_info: msg_sender_info,
                other_infos: other_infos,
                senders: pub_infos,
                msgs_and_sigs: msgs_and_sigs,
            }
        }

        fn num_nodes(&self) -> usize {
            self.senders.len()
        }
    }

    #[test]
    fn section_src_add_message_last() {
        let mut sig_accumulator = SignatureAccumulator::default();
        let env = Env::new();

        // Add all signatures for all messages - none should accumulate.
        env.msgs_and_sigs.iter().foreach(|msg_and_sigs| {
            msg_and_sigs
                .signature_msgs
                .iter()
                .zip(env.other_infos.iter())
                .foreach(|(signature_msg, full_info)| match *signature_msg {
                    DirectMessage::MessageSignature(ref hash, ref sig) => {
                        let result = sig_accumulator.add_signature(
                            env.num_nodes(),
                            *hash,
                            *sig,
                            *full_info.public_info(),
                        );
                        assert!(result.is_none());
                    }
                    ref unexpected_msg => panic!("Unexpected message: {:?}", unexpected_msg),
                });
        });

        assert!(sig_accumulator.msgs.is_empty());
        assert_eq!(sig_accumulator.sigs.len(), env.msgs_and_sigs.len());
        sig_accumulator.sigs.values().foreach(
            |&(ref pub_infos_and_sigs,
               _)| {
                assert_eq!(pub_infos_and_sigs.len(), env.other_infos.len())
            },
        );

        // Add each message with the section list added - each should accumulate.
        let mut expected_sigs_count = env.msgs_and_sigs.len();
        assert_eq!(sig_accumulator.sigs.len(), expected_sigs_count);
        assert!(sig_accumulator.msgs.is_empty());
        env.msgs_and_sigs.iter().foreach(|msg_and_sigs| {
            expected_sigs_count -= 1;
            let signed_msg = msg_and_sigs.signed_msg.clone();
            let route = rand::random();
            let (mut returned_msg, returned_route) = unwrap!(sig_accumulator.add_message(
                signed_msg.clone(),
                env.num_nodes(),
                route,
            ));
            assert_eq!(sig_accumulator.sigs.len(), expected_sigs_count);
            assert!(sig_accumulator.msgs.is_empty());
            assert_eq!(route, returned_route);
            assert_eq!(signed_msg.routing_message(), returned_msg.routing_message());
            unwrap!(returned_msg.check_integrity(1000));
            assert!(returned_msg.check_fully_signed(env.num_nodes()));
            env.senders.iter().foreach(|pub_info| {
                assert!(returned_msg.signed_by(pub_info))
            });
        });
    }

    #[test]
    fn section_src_add_signature_last() {
        let mut sig_accumulator = SignatureAccumulator::default();
        let env = Env::new();

        // Add each message with the section list added - none should accumulate.
        env.msgs_and_sigs.iter().enumerate().foreach(|(route,
          msg_and_sigs)| {
            let signed_msg = msg_and_sigs.signed_msg.clone();
            let result = sig_accumulator.add_message(signed_msg, env.num_nodes(), route as u8);
            assert!(result.is_none());
        });
        let mut expected_msgs_count = env.msgs_and_sigs.len();
        assert_eq!(sig_accumulator.msgs.len(), expected_msgs_count);
        assert!(sig_accumulator.sigs.is_empty());

        // Add each message's signatures - each should accumulate once quorum has been reached.
        env.msgs_and_sigs.iter().enumerate().foreach(|(route,
          msg_and_sigs)| {
            msg_and_sigs
                .signature_msgs
                .iter()
                .zip(env.other_infos.iter())
                .foreach(|(signature_msg, full_info)| {
                    let result = match *signature_msg {
                        DirectMessage::MessageSignature(hash, sig) => {
                            sig_accumulator.add_signature(
                                env.num_nodes(),
                                hash,
                                sig,
                                *full_info.public_info(),
                            )
                        }
                        ref unexpected_msg => panic!("Unexpected message: {:?}", unexpected_msg),
                    };

                    if let Some((mut returned_msg, returned_route)) = result {
                        expected_msgs_count -= 1;
                        assert_eq!(sig_accumulator.msgs.len(), expected_msgs_count);
                        assert_eq!(route, usize::from(returned_route));
                        assert_eq!(
                            msg_and_sigs.signed_msg.routing_message(),
                            returned_msg.routing_message()
                        );
                        unwrap!(returned_msg.check_integrity(1000));
                        assert!(returned_msg.check_fully_signed(env.num_nodes()));
                    }
                });
        });
    }
}
