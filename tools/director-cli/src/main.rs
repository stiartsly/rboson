/*
 * Copyright (c) 2023 -      bosonnetwork.io
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

//! `boson-director-cli`: the administration tool of a Boson super node.
//!
//! The command tree is the same as the Java `boson-director-cli`. None of the commands has a Rust
//! implementation yet; each is accepted and reports that it is not implemented.

use clap::{ArgMatches, Args, CommandFactory, Subcommand};

/// The administration tool of a Boson super node.
#[derive(clap::Parser, Debug)]
#[command(name = "boson-director-cli", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Manage the users of the node.
    #[command(subcommand)]
    User(UserCommand),
    /// Manage the devices of the node's users.
    #[command(subcommand)]
    Device(DeviceCommand),
    /// Manage the plans the node offers.
    #[command(subcommand)]
    Plan(PlanCommand),
    /// Manage what each plan grants on each service.
    #[command(subcommand)]
    Feature(FeatureCommand),
    /// Manage users' subscriptions to plans.
    #[command(subcommand)]
    Subscription(SubscriptionCommand),
    /// Manage the nodes and hosts this node refuses.
    #[command(subcommand)]
    Blacklist(BlacklistCommand),
    /// Inspect and manage this node's federation with other super nodes.
    #[command(subcommand)]
    Federation(FederationCommand),
    /// Show the super node's id and status.
    #[command(subcommand)]
    Node(NodeCommand),
    /// Show and change the configuration.
    #[command(subcommand)]
    Config(ConfigCommand),
    /// Create, import or show the identity this tool acts with.
    #[command(subcommand)]
    Identity(IdentityCommand),
}

/// The arguments of a command that is not implemented yet: accepted, and ignored.
#[derive(Args, Debug)]
struct Pending {
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, hide = true)]
    _args: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum UserCommand {
    /// List the users.
    List(Pending),
    /// Show a user's account.
    Show(Pending),
    /// Create a user account.
    Add(Pending),
    /// Change a user's account.
    Update(Pending),
    /// Remove a user's account.
    Remove(Pending),
    /// Make a user an administrator, with full control of the node.
    GrantAdmin(Pending),
    /// Take the administrator role from a user.
    RevokeAdmin(Pending),
    /// Make the user of a Boson Identity app an administrator of the node.
    EnrollAdmin(Pending),
}

#[derive(Subcommand, Debug)]
enum DeviceCommand {
    /// List a user's devices.
    List(Pending),
    /// Show a device.
    Show(Pending),
    /// Register a device to a user.
    Add(Pending),
    /// Remove a device. It can no longer act for its user.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum PlanCommand {
    /// List every plan, including the inactive ones.
    List(Pending),
    /// Show a plan.
    Show(Pending),
    /// Add a plan, billed monthly.
    Add(Pending),
    /// Change a plan.
    Update(Pending),
    /// Offer a plan to new subscribers.
    Activate(Pending),
    /// Withdraw a plan from new subscribers. Existing subscriptions are not affected.
    Deactivate(Pending),
}

#[derive(Subcommand, Debug)]
enum FeatureCommand {
    /// List the features, of every plan or of one.
    List(Pending),
    /// Show a feature and its document.
    Show(Pending),
    /// Set what a plan grants on a service.
    Add(Pending),
    /// Change a feature: its service, its document, or both.
    Update(Pending),
    /// Remove a feature. Its plan gets the service's defaults.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum SubscriptionCommand {
    /// List a user's subscriptions, past and present.
    List(Pending),
    /// Show a subscription.
    Show(Pending),
    /// Show a user's active subscription.
    Active(Pending),
    /// Subscribe a user to a plan.
    Add(Pending),
    /// Change a subscription's state, end or plan.
    Update(Pending),
    /// Cancel a subscription.
    Cancel(Pending),
}

#[derive(Subcommand, Debug)]
enum BlacklistCommand {
    /// List the blacklist.
    List(Pending),
    /// Show a blacklist entry.
    Show(Pending),
    /// Add a node or a host to the blacklist.
    Add(Pending),
    /// Change a blacklist entry's reason, or whether it is automatic.
    Update(Pending),
    /// Remove a node or a host from the blacklist.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum FederationCommand {
    /// The nodes this node has federated with.
    #[command(subcommand)]
    Node(FederationNodeCommand),
    /// The services federated nodes share with this node.
    #[command(subcommand)]
    Service(FederationServiceCommand),
    /// The federation proposals this node has made and received.
    #[command(subcommand)]
    Proposal(FederationProposalCommand),
    /// Propose federation to another super node.
    Propose(Pending),
}

#[derive(Subcommand, Debug)]
enum FederationNodeCommand {
    /// List the federated nodes.
    List(Pending),
    /// Show a federated node.
    Show(Pending),
    /// Suspend or resume the federation with a node.
    Update(Pending),
    /// Remove a node from this node's federation.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum FederationServiceCommand {
    /// List the services a federated node shares with this node.
    List(Pending),
}

#[derive(Subcommand, Debug)]
enum FederationProposalCommand {
    /// List the federation proposals.
    List(Pending),
    /// Show a federation proposal.
    Show(Pending),
    /// Remove a federation proposal from the record.
    Remove(Pending),
}

#[derive(Subcommand, Debug)]
enum NodeCommand {
    /// Show the super node's id, as its Director reports it.
    Id(Pending),
    /// Show what the super node runs and which services it offers.
    Status(Pending),
}

#[derive(Subcommand, Debug)]
enum ConfigCommand {
    /// Create the configuration file.
    Init(Pending),
    /// Show the settings in effect, and where each one comes from.
    Show(Pending),
    /// Change a setting in the configuration file.
    Set(Pending),
    /// Remove a setting from the configuration file.
    Unset(Pending),
}

#[derive(Subcommand, Debug)]
enum IdentityCommand {
    /// Create a new identity file with a new key.
    Create(Pending),
    /// Create the identity file from an existing private key.
    Import(Pending),
    /// Show the id of the identity in effect, and where it comes from.
    Show(Pending),
}

fn main() {
    let matches = Cli::command().get_matches();
    eprintln!("boson-director-cli {}: not implemented yet", command_path(&matches));
    std::process::exit(2);
}

/// The subcommand names the user typed, such as `federation node list`.
fn command_path(matches: &ArgMatches) -> String {
    let mut names = Vec::new();
    let mut current = matches;
    while let Some((name, sub)) = current.subcommand() {
        names.push(name);
        current = sub;
    }
    names.join(" ")
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    #[test]
    fn command_tree_is_valid() {
        super::Cli::command().debug_assert();
    }

    #[test]
    fn command_path_names_the_subcommands() {
        let matches = super::Cli::command()
            .try_get_matches_from(["boson-director-cli", "federation", "node", "list", "-x"])
            .unwrap();
        assert_eq!(super::command_path(&matches), "federation node list");
    }
}
