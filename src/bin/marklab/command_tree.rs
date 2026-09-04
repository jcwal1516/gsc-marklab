use std::collections::BTreeMap;

use clap::{Command, CommandFactory};

type Handler = fn() -> marklab::Result<()>;

/// One existing typed argument decoder and its execution handler.
pub(crate) struct Route {
    command: Command,
    handler: Handler,
}

impl Route {
    pub(crate) fn new<C: CommandFactory>(handler: Handler) -> Self {
        Self {
            command: C::command(),
            handler,
        }
    }
}

pub(crate) fn run() -> marklab::Result<()> {
    let mut command = marklab::cli_command();
    let mut handlers = BTreeMap::new();
    register_leaves(&command, &mut Vec::new(), marklab::run_cli, &mut handlers)?;
    let routes = [
        super::backend::cli_route(),
        super::project::cli_route(),
        super::cohort::cli_route(),
        super::bayes_advanced::cli_route(),
        super::longitudinal::cli_route(),
        super::spatial3d_model::cli_route(),
        super::spatial3d::cli_route(),
        super::causal_model::cli_route(),
        super::causal::cli_route(),
        super::numerics::cli_route(),
        super::policy::cli_route(),
        super::graph::cli_route(),
        super::topology::cli_route(),
        super::registration::cli_route(),
        super::neural::cli_route(),
        super::multimodal_model::cli_route(),
    ];
    for route in routes.into_iter().chain(super::bayes::cli_routes()) {
        register_leaves(
            &route.command,
            &mut Vec::new(),
            route.handler,
            &mut handlers,
        )?;
        command = merge_commands(command, route.command);
    }

    let matches = command.get_matches();
    let mut path = Vec::new();
    let mut current = &matches;
    while let Some((name, nested)) = current.subcommand() {
        path.push(name.to_owned());
        current = nested;
    }
    let handler = handlers.get(&path).ok_or_else(|| {
        marklab::MarklabError::Validation(format!(
            "no execution handler for parsed command {}",
            path.join(" ")
        ))
    })?;
    handler()
}

fn register_leaves(
    command: &Command,
    path: &mut Vec<String>,
    handler: Handler,
    handlers: &mut BTreeMap<Vec<String>, Handler>,
) -> marklab::Result<()> {
    if !command.has_subcommands() {
        if handlers.insert(path.clone(), handler).is_some() {
            return Err(marklab::MarklabError::Validation(format!(
                "duplicate CLI execution owner for {}",
                path.join(" ")
            )));
        }
        return Ok(());
    }
    for child in command.get_subcommands() {
        path.push(child.get_name().to_owned());
        register_leaves(child, path, handler, handlers)?;
        path.pop();
    }
    Ok(())
}

fn merge_commands(mut command: Command, extension: Command) -> Command {
    for child in extension.get_subcommands() {
        if let Some(existing) = command
            .get_subcommands_mut()
            .find(|existing| existing.get_name() == child.get_name())
        {
            *existing = merge_commands(existing.clone(), child.clone());
            continue;
        }
        command = command.subcommand(child.clone());
    }
    command
}
