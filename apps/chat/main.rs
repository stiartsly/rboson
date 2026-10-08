use boson::{
    core::logger,
    errors::Result,
    Id,
    messaging::{
        Channel, ChannelListener, Client, ConnectionListener, Contact, ContactListener,
        FriendRequestListener, Message, MessageListener, Options as MessagingOptions,
        OptionsBuilder as MessagingOptionsBuilder, SessionInfo, SessionListener,
    },
};
use clap::Parser;
use reedline::{ExternalPrinter, Reedline, Signal};
use std::{
    io::{self, Write},
    sync::Arc,
};

mod cmds;
mod prompt;
use prompt::MyPrompt;

#[derive(Parser, Debug)]
#[command(name = "chat", version = "1.0", about = "Photon chat")]
struct Options {
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    config: String,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("chat: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let options = Options::parse();
    let external_printer = ExternalPrinter::new(4_096);
    let output = ConsoleOutput::new(external_printer.clone());

    let printer_for_cmds = external_printer.clone();
    cmds::set_result_output(move |text| {
        for line in text.lines() {
            let _ = printer_for_cmds.sender().send(line.to_string());
        }
        if text.is_empty() {
            let _ = printer_for_cmds.sender().send(String::new());
        }
    });

    let chat_options = load_chat_options(&options, &output)?;
    let client = create_client(chat_options, &external_printer);

    client.start().await?;

    let mut cli = build_cli();
    let mut editor = Reedline::create().with_external_printer(external_printer);
    let prompt = MyPrompt;
    cmds::print_result("Welcome to the messaging shell. Type 'help' or 'exit'.".to_string());

    loop {
        output.flush_pending(&mut io::stderr().lock())?;
        match editor.read_line(&prompt) {
            Ok(Signal::Success(line)) => {
                let args = parse_command_line(&line);
                if args.is_empty() {
                    continue;
                }
                if matches!(args[0].as_str(), "exit" | "quit") {
                    break;
                }
                if args[0] == "help" {
                    let mut buf = Vec::new();
                    if let Some(name) = args.get(1) {
                        match cli.find_subcommand_mut(name) {
                            Some(command) => command.write_long_help(&mut buf)?,
                            _ => cli.write_long_help(&mut buf)?,
                        }
                    } else {
                        cli.write_long_help(&mut buf)?;
                    }
                    let s = String::from_utf8_lossy(&buf);
                    cmds::print_result(s.to_string());
                    continue;
                }
                match cli.clone().try_get_matches_from(args) {
                    Ok(matches) if matches.subcommand_name() == Some("clear") => {
                        output.flush_pending(&mut io::stderr().lock())?;
                        editor.clear_screen()?;
                    }
                    Ok(matches) => cmds::execute_command(matches, &client).await,
                    Err(error) => cmds::print_result(error.to_string()),
                }
            }
            Ok(Signal::CtrlC | Signal::CtrlD) => break,
            Ok(_) => continue,
            Err(error) => {
                cmds::print_result(format!("Input error: {error}"));
                break;
            }
        }
    }
    client.stop().await?;
    Ok(())
}

fn build_cli() -> clap::Command {
    cmds::build_cli().subcommand(
        clap::Command::new("clear")
            .about("Clear the terminal screen and redraw the prompt"),
    )
}

fn create_client(
    options: MessagingOptions,
    external_printer: &ExternalPrinter<String>,
) -> Arc<Client> {
    let client = Arc::new(Client::new(options));
    let log_sender = external_printer.sender();
    logger::set_console_output_handler(move |line| {
        for l in line.lines() {
            _ = log_sender.send(l.trim_start_matches([' ', '\t', '\r']).to_string());
        }
    });
    client
}

fn load_chat_options(options: &Options, output: &ConsoleOutput) -> Result<MessagingOptions> {
    MessagingOptionsBuilder::load(&options.config)?
        .with_connection_listener(PhotonConnectionListener::new(output.clone()))
        .with_message_listener(PhotonMessageListener::new(output.clone()))
        .with_channel_listener(PhotonChannelListener::new(output.clone()))
        .with_contact_listener(PhotonContactListener::new(output.clone()))
        .with_session_listener(PhotonSessionListener::new(output.clone()))
        .with_friend_request_listener(PhotonFriendRequestListener::new(output.clone()))
        .build()
}

#[derive(Clone)]
struct ConsoleOutput {
    printer: ExternalPrinter<String>,
}

impl ConsoleOutput {
    fn new(printer: ExternalPrinter<String>) -> Self {
        Self { printer }
    }

    fn println(&self, line: impl Into<String>) {
        let text = line.into();
        for l in text.lines() {
            let _ = self.printer.sender().send(l.to_string());
        }
    }

    fn flush_pending(&self, writer: &mut impl Write) -> io::Result<()> {
        // Between read_line calls, flush output before Reedline measures the cursor.
        // Its external printer can otherwise detect a reset from unflushed lines.
        for line in self.printer.receiver().try_iter() {
            writeln!(writer, "{line}")?;
        }
        writer.flush()
    }
}

struct PhotonConnectionListener {
    output: ConsoleOutput,
}

impl PhotonConnectionListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ConnectionListener for PhotonConnectionListener {
    fn on_connecting(&self) {
        self.output.println("Connecting to messaging service...");
    }

    fn on_connected(&self) {
        self.output.println("Connected to messaging service");
    }

    fn on_ready(&self) {
        self.output.println("Messaging service is ready");
    }

    fn on_disconnected(&self) {
        self.output.println("Disconnected from messaging service");
    }
}

struct PhotonMessageListener {
    output: ConsoleOutput,
}

impl PhotonMessageListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl MessageListener for PhotonMessageListener {
    fn on_message(&self, message: &dyn Message) {
        let from = message.from().map(ToString::to_string).unwrap_or_else(|| "unknown".into());
        let content = message
            .payload_as_content()
            .and_then(|content| content.as_text())
            .unwrap_or("<binary message>");
        self.output.println(format!("Message from {from}: {content}"));
    }

    fn on_sent(&self, message: &dyn Message) {
        self.output
            .println(format!("Sent message {}", message.id()));
    }
}

fn parse_command_line(line: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in line.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if let Some(delimiter) = quote {
            if character == delimiter {
                quote = None;
            } else {
                current.push(character);
            }
        } else if character == '"' || character == '\'' {
            quote = Some(character);
        } else if character.is_whitespace() {
            if !current.is_empty() {
                args.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if escaped {
        current.push('\\');
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

struct PhotonContactListener {
    output: ConsoleOutput,
}

impl PhotonContactListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ContactListener for PhotonContactListener {
    fn on_contact_added(&self, contact: &dyn Contact) {
        self.output
            .println(format!("Contact added: {}", contact.id()));
    }

    fn on_contacts_updated(&self, contacts: &[Box<dyn Contact>]) {
        self.output
            .println(format!("Updated {} contact(s)", contacts.len()));
    }

    fn on_contacts_removed(&self, contact_ids: &[Id]) {
        self.output
            .println(format!("Removed {} contact(s)", contact_ids.len()));
    }

    fn on_contacts_cleared(&self) {
        self.output.println("Contacts cleared");
    }
}

struct PhotonChannelListener {
    output: ConsoleOutput,
}

impl PhotonChannelListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl ChannelListener for PhotonChannelListener {
    fn on_channel_created(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Channel created: {}", channel.id()));
    }

    fn on_channel_deleted(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Channel deleted: {}", channel.id()));
    }

    fn on_joined_channel(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Joined channel: {}", channel.id()));
    }

    fn on_left_channel(&self, channel: &dyn Channel) {
        self.output
            .println(format!("Left channel: {}", channel.id()));
    }
}

struct PhotonSessionListener {
    output: ConsoleOutput,
}

impl PhotonSessionListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl SessionListener for PhotonSessionListener {
    fn on_new_session(&self, session: &SessionInfo) {
        self.output
            .println(format!("New device session: {}", session.device_id()));
    }
}

struct PhotonFriendRequestListener {
    output: ConsoleOutput,
}

impl PhotonFriendRequestListener {
    fn new(output: ConsoleOutput) -> Self {
        Self { output }
    }
}

impl FriendRequestListener for PhotonFriendRequestListener {
    fn on_friend_request(&self, user_id: &Id, hello: Option<&str>) {
        self.output.println(format!(
            "Friend request from {}: {}",
            user_id,
            hello.unwrap_or("<no greeting>")
        ));
    }

    fn on_friend_request_accepted(&self, user_id: &Id) {
        self.output
            .println(format!("Friend request accepted by {user_id}"));
    }
}
