use crate::completions::{file_path_completion, Completer, CompletionOptions};
use nu_protocol::{
    engine::{Stack, StateWorkingSet},
    Span,
};
use reedline::Suggestion;
use std::path::{is_separator, PathBuf, MAIN_SEPARATOR as SEP, MAIN_SEPARATOR_STR};

use super::{SemanticSuggestion, SuggestionKind};

#[derive(Clone, Default)]
pub struct DotNuCompletion {}

impl DotNuCompletion {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Completer for DotNuCompletion {
    fn fetch(
        &mut self,
        working_set: &StateWorkingSet,
        stack: &Stack,
        prefix: &[u8],
        span: Span,
        offset: usize,
        _pos: usize,
        options: &CompletionOptions,
    ) -> Vec<SemanticSuggestion> {
        let prefix_str = String::from_utf8_lossy(prefix).replace('`', "");
        let mut search_dirs: Vec<PathBuf> = vec![];

        // If prefix_str is only a word we want to search in the current dir
        let (base, partial) = prefix_str
            .rsplit_once(is_separator)
            .unwrap_or((".", &prefix_str));
        let base_dir = base.replace(is_separator, MAIN_SEPARATOR_STR);

        // Fetch the lib dirs
        let lib_dirs: Vec<PathBuf> = working_set
            .find_variable(b"$NU_LIB_DIRS")
            .and_then(|vid| working_set.get_variable(vid).const_val.as_ref())
            .or(working_set.get_env_var("NU_LIB_DIRS"))
            .map(|lib_dirs| {
                lib_dirs
                    .as_list()
                    .into_iter()
                    .flat_map(|it| {
                        it.iter().map(|x| {
                            x.to_path()
                                .expect("internal error: failed to convert lib path")
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        // Check if the base_dir is a folder
        // rsplit_once removes the separator
        let cwd = working_set.permanent_state.cwd(None);
        if base_dir != "." {
            // Search in base_dir as well as lib_dirs
            if let Ok(mut cwd) = cwd {
                cwd.push(&base_dir);
                search_dirs.push(cwd.into_std_path_buf());
            }
            search_dirs.extend(lib_dirs.into_iter().map(|mut dir| {
                dir.push(&base_dir);
                dir
            }));
        } else {
            if let Ok(cwd) = cwd {
                search_dirs.push(cwd.into_std_path_buf());
            }
            search_dirs.extend(lib_dirs);
        }

        // Fetch the files filtering the ones that ends with .nu
        // and transform them into suggestions

        let completions = file_path_completion(
            span,
            partial,
            &search_dirs
                .iter()
                .map(|d| d.to_str().unwrap_or_default())
                .collect::<Vec<_>>(),
            options,
            working_set.permanent_state,
            stack,
        );
        completions
            .into_iter()
            // Different base dir, so we list the .nu files or folders
            .filter(|it| it.path.ends_with(".nu") || it.path.ends_with(SEP))
            .map(|x| {
                let append_whitespace = x.path.ends_with(".nu");
                // Re-calculate the span to replace
                let span_offset = if base_dir == "." {
                    0
                } else {
                    base_dir.len() + 1
                } + prefix.iter().take_while(|c| **c == b'`').count();
                let end = x.span.end - offset;
                let start = std::cmp::min(end, x.span.start - offset + span_offset);
                SemanticSuggestion {
                    suggestion: Suggestion {
                        value: x.path,
                        style: x.style,
                        span: reedline::Span { start, end },
                        append_whitespace,
                        ..Suggestion::default()
                    },
                    kind: Some(SuggestionKind::Module),
                }
            })
            .collect::<Vec<_>>()
    }
}
