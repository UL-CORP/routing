initSidebarItems({"constant":[["XOR_NAME_LEN","Constant byte length of `XorName`."]],"enum":[["DstLocation","Message destination location."],["RoutingError","Internal error."],["SrcLocation","Message source location."],["TransportEvent","QuicP2p Events to the user"]],"macro":[["debug","Log a message at the debug level prefixed with the current log ident."],["error","Log a message at the error level prefixed with the current log ident."],["info","Log a message at the info level prefixed with the current log ident."],["log","Log a message at the given level prefixed with the current log ident."],["log_or_panic","This macro will panic with the given message if compiled with \"mock_base\", otherwise it will simply log the message at the requested level."],["trace","Log a message at the trace level prefixed with the current log ident."],["warn","Log a message at the warn level prefixed with the current log ident."]],"mod":[["event","Routing events."]],"struct":[["FullId","Network identity component containing name, and public and private keys."],["NetworkParams","Network parameters: number of elders, recommended section size"],["Node","Interface for sending and receiving messages to and from other nodes, in the role of a full routing node."],["NodeConfig","Node configuration."],["P2pNode","Network p2p node identity. When a node knows another node as a `P2pNode` it's implicitly connected to it. This is separate from being connected at the network layer, which currently is handled by quic-p2p."],["PausedState","A type that wraps the internal state of a node while it is paused in order to be upgraded and/or restarted. A value of this type is obtained by pausing a node and can be then used to resume it."],["Prefix","A section prefix, i.e. a sequence of bits specifying the part of the network's name space consisting of all names that start with this sequence."],["PublicId","Network identity component containing name and public keys."],["SectionProofChain","Chain of section BLS keys where every key is proven (signed) by the previous key, except the first one."],["TransportConfig","QuicP2p configurations"],["XorName","A 256-bit number, viewed as a point in XOR space."]]});