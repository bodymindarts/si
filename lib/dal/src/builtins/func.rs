use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use regex::Regex;
use serde::{Deserialize, Serialize};
use telemetry::prelude::*;

use crate::BuiltinsError::SerdeJson;
use crate::{
    func::argument::{FuncArgument, FuncArgumentKind},
    BuiltinsError, BuiltinsResult, DalContext, Func, FuncBackendKind, FuncBackendResponseType,
    StandardModel,
};

#[derive(Deserialize, Serialize, Debug)]
struct FunctionMetadataArgument {
    name: String,
    kind: FuncArgumentKind,
}

#[derive(Deserialize, Serialize, Debug)]
struct FunctionMetadata {
    #[serde(skip)]
    name: String,
    kind: FuncBackendKind,
    arguments: Option<Vec<FunctionMetadataArgument>>,
    response_type: FuncBackendResponseType,
    hidden: Option<bool>,
    display_name: Option<String>,
    description: Option<String>,
    link: Option<String>,
    code_file: Option<String>,
    code_entrypoint: Option<String>,
}

impl FunctionMetadata {
    pub async fn from_func(ctx: &DalContext, f: &Func) -> Self {
        let func_name_regex = Regex::new(r"si:(?P<name>.*)").unwrap();
        let func_name = func_name_regex
            .captures(f.name())
            .unwrap()
            .name("name")
            .unwrap()
            .as_str();

        let extension = match f.backend_kind() {
            FuncBackendKind::JsAttribute
            | FuncBackendKind::JsWorkflow
            | FuncBackendKind::JsCommand => Some("js"),

            _ => None,
        };

        let code_file = extension.map(|e| format!("{}.{}", func_name, e));

        let arguments = Some(
            FuncArgument::list_for_func(ctx, *f.id())
                .await
                .expect("could not list function arguments")
                .iter()
                .map(|arg| FunctionMetadataArgument {
                    name: arg.name().to_string(),
                    kind: *arg.kind(),
                })
                .collect(),
        );

        FunctionMetadata {
            arguments,
            name: func_name.to_string(),
            kind: *f.backend_kind(),
            response_type: *f.backend_response_type(),
            hidden: Some(f.hidden()),
            // TODO Convert FunctionMetadata fields to use &str and remove these maps
            display_name: f.display_name().map(|s| s.to_string()),
            description: f.description().map(|s| s.to_string()),
            link: f.link().map(|s| s.to_string()),
            code_file,
            code_entrypoint: f.handler().map(|s| s.to_string()),
        }
    }
}

/// We want the src/builtins/func/** files to be available at run time inside of the Docker container
/// that we build, but it would be nice to not have to include arbitrary bits of the source tree when
/// building it. This allows us to compile the builtins into the binary, so they're already available
/// in memory.
///
/// The instances of this end up in a magic `ASSETS` array const.
#[iftree::include_file_tree("paths = '/src/builtins/func/**'")]
pub struct FuncBuiltin {
    relative_path: &'static str,
    contents_str: &'static str,
}

static FUNC_BUILTIN_BY_PATH: once_cell::sync::Lazy<std::collections::HashMap<&str, &FuncBuiltin>> =
    once_cell::sync::Lazy::new(|| {
        ASSETS
            .iter()
            .map(|func_builtin| (func_builtin.relative_path, func_builtin))
            .collect()
    });

pub async fn migrate(ctx: &DalContext) -> BuiltinsResult<()> {
    for builtin_func_file in ASSETS.iter() {
        let builtin_path = std::path::Path::new(builtin_func_file.relative_path);
        match builtin_path.extension() {
            Some(extension) => {
                if extension != std::ffi::OsStr::new("json") {
                    debug!("skipping {:?}: not a json file", builtin_path);
                    continue;
                }
            }
            None => {
                warn!("skipping {:?}: no file extension", builtin_path);
                continue;
            }
        };

        let func_metadata: FunctionMetadata = serde_json::from_str(builtin_func_file.contents_str)?;

        let func_name = format!(
            "si:{}",
            builtin_path
                .file_stem()
                .ok_or_else(|| {
                    BuiltinsError::FuncMetadata(format!(
                        "Unable to determine base file name for {:?}",
                        builtin_path
                    ))
                })?
                .to_string_lossy()
        );

        let existing_func = Func::find_by_attr(ctx, "name", &func_name).await?;
        if !existing_func.is_empty() {
            warn!("skipping {:?}: func already exists", &func_name);
            continue;
        }

        let mut new_func = Func::new(
            ctx,
            &func_name,
            func_metadata.kind,
            func_metadata.response_type,
        )
        .await
        .expect("cannot create func");

        if let Some(code_file) = func_metadata.code_file {
            if func_metadata.code_entrypoint.is_none() {
                panic!("cannot create function with code_file but no code_entrypoint")
            }

            let metadata_base_path = builtin_path.parent().ok_or_else(|| {
                BuiltinsError::FuncMetadata(format!(
                    "Cannot determine parent path of {:?}",
                    builtin_path
                ))
            })?;
            let func_path = metadata_base_path.join(std::path::Path::new(&code_file));

            let code = FUNC_BUILTIN_BY_PATH
                .get(func_path.as_os_str().to_str().ok_or_else(|| {
                    BuiltinsError::FuncMetadata(format!(
                        "Unable to convert {:?} to &str",
                        func_path
                    ))
                })?)
                .ok_or_else(|| {
                    BuiltinsError::FuncMetadata(format!("Code file not found: {:?}", code_file))
                })?;
            let code = base64::encode(code.contents_str);
            new_func
                .set_code_base64(ctx, Some(code))
                .await
                .expect("cannot set code");
        }

        new_func
            .set_handler(ctx, func_metadata.code_entrypoint)
            .await
            .expect("cannot set handler");

        new_func
            .set_display_name(ctx, func_metadata.display_name)
            .await
            .expect("cannot set display name");
        new_func
            .set_description(ctx, func_metadata.description)
            .await
            .expect("cannot set func description");
        new_func
            .set_link(ctx, func_metadata.link)
            .await
            .expect("cannot set func link");
        new_func
            .set_hidden(ctx, func_metadata.hidden.unwrap_or(false))
            .await
            .expect("cannot set func hidden");

        if let Some(arguments) = func_metadata.arguments {
            for arg in arguments {
                FuncArgument::new(ctx, &arg.name, arg.kind, None, *new_func.id()).await?;
            }
        }
    }

    Ok(())
}

/// A private constant representing "/si/lib/dal".
const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub async fn persist(ctx: &DalContext, func: &Func) -> BuiltinsResult<()> {
    let new_metadata: FunctionMetadata = FunctionMetadata::from_func(ctx, func).await;

    let mut base_path = PathBuf::from(CARGO_MANIFEST_DIR);
    base_path.push("src/builtins/func");

    if let Some(code_path) = new_metadata.code_file.as_ref() {
        let mut code_file_path = base_path.clone();
        code_file_path.push(code_path);

        let mut code_file = File::create(code_file_path)?;

        code_file.write_all(func.code_plaintext()?.unwrap().as_bytes())?;
    }

    let mut metadata_path = base_path.clone();
    metadata_path.push(format!("{}.json", new_metadata.name));
    let metadata_file = File::create(metadata_path)?;

    serde_json::to_writer_pretty(metadata_file, &new_metadata).map_err(SerdeJson)
}
