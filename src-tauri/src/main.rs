fn main() {
    if std::env::args().any(|argument| argument == "--mcp-server") {
        agentdeck_lib::run_mcp_server();
        return;
    }

    agentdeck_lib::run()
}
