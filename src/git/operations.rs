use std::path::{Path, PathBuf};
use chrono::{DateTime, TimeZone, Utc};
use git2::{Repository, Branch, DiffOptions, Oid};
use git2::build::RepoBuilder;
use tokio::time::{Instant, Duration};
use tracing::error;

use crate::custom_error::MapErrToString;
use crate::files_correction::to_pathbuf_normalize;
use super::{FileChange, FileChangeStatus};

fn status_options(include_untracked: bool, include_unmodified: bool, show_index_only: bool) -> git2::StatusOptions {
    let mut options = git2::StatusOptions::new();
    options
        .disable_pathspec_match(true)
        .include_ignored(false)
        .include_unmodified(include_unmodified)
        .include_unreadable(false)
        .include_untracked(include_untracked)
        .recurse_ignored_dirs(false)
        .recurse_untracked_dirs(include_untracked)
        .rename_threshold(100)
        .update_index(true)
        .show(if show_index_only { git2::StatusShow::Index } else { git2::StatusShow::IndexAndWorkdir });
    options
}

pub fn git_ls_files(repository_path: &PathBuf) -> Option<Vec<PathBuf>> {
    let repository = Repository::open(repository_path)
        .map_err(|e| error!("Failed to open repository: {}", e)).ok()?;

    let statuses = repository.statuses(Some(&mut status_options(true, true, false)))
        .map_err(|e| error!("Failed to get statuses: {}", e)).ok()?;

    let mut files = Vec::new();
    for entry in statuses.iter() {
        let path = String::from_utf8_lossy(entry.path_bytes()).to_string();
        files.push(repository_path.join(path));
    }
    if !files.is_empty() { Some(files) } else { None }
}

pub fn get_or_create_branch<'repo>(repository: &'repo Repository, branch_name: &str) -> Result<Branch<'repo>, String> {
    match repository.find_branch(branch_name, git2::BranchType::Local) {
        Ok(branch) => Ok(branch),
        Err(_) => {
            let head_commit = repository.head()
                .and_then(|h| h.peel_to_commit())
                .map_err_with_prefix("Failed to get HEAD commit:")?;
            repository.branch(branch_name, &head_commit, false)
                .map_err_with_prefix("Failed to create branch:")
        }
    }
}

pub fn get_diff_statuses_index_to_head(repository: &Repository, include_untracked: bool) -> Result<Vec<FileChange>, String> {
    let repository_workdir = repository.workdir()
        .ok_or("Failed to get workdir from repository".to_string())?;
    
    let mut result = Vec::new();
    let statuses = repository.statuses(Some(&mut status_options(include_untracked, false, true)))
        .map_err_with_prefix("Failed to get statuses:")?;
    for entry in statuses.iter() {
        let status = entry.status();
        let relative_path = PathBuf::from(String::from_utf8_lossy(entry.path_bytes()).to_string());
        let absolute_path = to_pathbuf_normalize(&repository_workdir.join(&relative_path).to_string_lossy());

        match status {
            s if s.is_ignored() || s.is_wt_renamed() || s.is_wt_renamed() || s.is_wt_deleted() || 
                s.is_wt_modified() || s.is_wt_new() || s.is_wt_typechange() => {
                tracing::error!("File status is {:?} for file {:?}, which should not be present due to status options.", status, relative_path)
            },
            s if s.is_index_new() => result.push(FileChange {
                status: FileChangeStatus::ADDED,
                relative_path: relative_path.clone(),
                absolute_path: absolute_path.clone(),
            }),
            s if s.is_index_modified() || s.is_index_typechange() || s.is_conflicted() => result.push(FileChange {
                status: FileChangeStatus::MODIFIED,
                relative_path: relative_path.clone(),
                absolute_path: absolute_path.clone(),
            }),
            s if s.is_index_deleted() => result.push(FileChange {
                status: FileChangeStatus::DELETED,
                relative_path: relative_path.clone(),
                absolute_path: absolute_path.clone(),
            }),
            _ => (),
        };
    }

    Ok(result)
}

pub fn get_diff_statuses_workdir_to_head(repository: &Repository, include_untracked: bool) -> Result<Vec<FileChange>, String> {
    let repository_workdir = repository.workdir()
        .ok_or("Failed to get workdir from repository".to_string())?;

    let head = repository.head().map_err_with_prefix("Failed to get HEAD:")?;
    let tree = head.peel_to_tree().map_err_with_prefix("Failed to get HEAD tree:")?;

    let mut diff_opts = git2::DiffOptions::new();
    diff_opts
        .include_untracked(include_untracked)
        .recurse_untracked_dirs(include_untracked)
        .show_untracked_content(include_untracked)
        .include_ignored(false)
        .include_unmodified(false)
        .update_index(true)
        .include_unreadable(false)
        .recurse_ignored_dirs(false)
        .disable_pathspec_match(true)
        .include_typechange(false)
        .show_binary(false);

    let diff = repository.diff_tree_to_workdir(Some(&tree), Some(&mut diff_opts))
        .map_err_with_prefix("Failed to get diff:")?;

    let mut result = Vec::new();
    diff.print(git2::DiffFormat::NameStatus, |_delta, _hunk, line| {
        // Format is "X\tpath" where X is status code
        let line_content = String::from_utf8_lossy(line.content()).to_string();
        if let Some((status_str, path)) = line_content.split_once('\t') {
            let status = match status_str {
                "A" | "?" => Some(FileChangeStatus::ADDED),
                "D" => Some(FileChangeStatus::DELETED),
                "M" | "T" | "U" => Some(FileChangeStatus::MODIFIED),
                "R" | "C" | " " | "!" | "X" => {
                    tracing::error!("Status {status_str} found for {path}, which should not be present due to status options.");
                    None
                },
                _ => {
                    tracing::error!("Unknown status {status_str} found for {path}.");
                    None
                }
            };

            if let Some(status) = status {
                let relative_path = PathBuf::from(path.trim());
                let absolute_path = to_pathbuf_normalize(&repository_workdir.join(&relative_path).to_string_lossy());
                result.push(FileChange {
                    status,
                    relative_path,
                    absolute_path,
                });
            }
        }
        true
    }).map_err_with_prefix("Failed to process diff:")?;

    Ok(result)
}

pub fn get_diff_statuses_index_to_commit(repository: &Repository, include_untracked: bool, commit_oid: &git2::Oid) -> Result<Vec<FileChange>, String> {
    let head = repository.head().map_err_with_prefix("Failed to get HEAD:")?;
    let original_head_ref = head.is_branch().then(|| head.name().map(ToString::to_string)).flatten();
    let original_head_oid = head.target();

    repository.set_head_detached(commit_oid.clone()).map_err_with_prefix("Failed to set HEAD:")?;

    let result = get_diff_statuses_index_to_head(repository, include_untracked);

    let restore_result = match (&original_head_ref, original_head_oid) {
        (Some(head_ref), _) => repository.set_head(head_ref),
        (None, Some(oid)) => repository.set_head_detached(oid),
        (None, None) => Ok(()),
    };
    
    if let Err(restore_err) = restore_result {
        let prev_err = result.as_ref().err().cloned().unwrap_or_default();
        return Err(format!("{}\nFailed to restore head: {}", prev_err, restore_err));
    }

    result
}

pub fn stage_changes(repository: &Repository, file_changes: &Vec<FileChange>) -> Result<(), String> {
    let mut index = repository.index().map_err_with_prefix("Failed to get index:")?;

    for file_change in file_changes {
        match file_change.status {
            FileChangeStatus::ADDED | FileChangeStatus::MODIFIED => {
                index.add_path(&file_change.relative_path)
                    .map_err_with_prefix("Failed to add file to index:")?;
            },
            FileChangeStatus::DELETED => {
                index.remove_path(&file_change.relative_path)
                    .map_err_with_prefix("Failed to remove file from index:")?;
            },
        }
    }

    index.write().map_err_with_prefix("Failed to write index:")?;
    Ok(())
}

pub fn get_configured_author_email_and_name(repository: &Repository) -> Result<(String, String), String> {
    let config = repository.config()
        .map_err_with_prefix("Failed to get repository config:")?;
    let author_email = config.get_string("user.email")
        .map_err_with_prefix("Failed to get author email:")?;
    let author_name = config.get_string("user.name")
        .map_err_with_prefix("Failed to get author name:")?;
    Ok((author_email, author_name))
}

pub fn commit(repository: &Repository, branch: &Branch, message: &str, author_name: &str, author_email: &str) -> Result<Oid, String> {
    let mut index = repository.index().map_err_with_prefix("Failed to get index:")?;
    let tree_id = index.write_tree().map_err_with_prefix("Failed to write tree:")?;
    let tree = repository.find_tree(tree_id).map_err_with_prefix("Failed to find tree:")?;

    let signature = git2::Signature::now(author_name, author_email)
        .map_err_with_prefix("Failed to create signature:")?;
    let branch_ref_name = branch.get().name().ok_or("Invalid branch name".to_string())?;

    let parent_commit = if let Some(target) = branch.get().target() {
        repository.find_commit(target)
            .map_err(|e| format!("Failed to find branch commit: {}", e))?
    } else {
        return Err("No parent commits found".to_string());
    };

    let commit = repository.commit(
        Some(branch_ref_name), &signature, &signature, message, &tree, &[&parent_commit]
    ).map_err(|e| format!("Failed to create commit: {}", e))?;

    repository.set_head(branch_ref_name).map_err_with_prefix("Failed to set branch as head:")?;

    Ok(commit)
}

pub fn get_commit_datetime(repository: &Repository, commit_oid: &Oid) -> Result<DateTime<Utc>, String> {
    let commit = repository.find_commit(commit_oid.clone()).map_err_to_string()?;

    Utc.timestamp_opt(commit.time().seconds(), 0).single()
        .ok_or_else(|| "Failed to get commit datetime".to_string())
}

pub fn git_diff<'repo>(repository: &'repo Repository, file_changes: &Vec<FileChange>) -> Result<git2::Diff<'repo>, String> {
    let mut diff_options = DiffOptions::new();
    diff_options.include_untracked(true);
    diff_options.recurse_untracked_dirs(true);
    for file_change in file_changes {
        diff_options.pathspec(&file_change.relative_path);
    }

    let mut sorted_file_changes = file_changes.clone();
    sorted_file_changes.sort_by_key(|fc| {
        std::fs::metadata(&fc.relative_path).map(|meta| meta.len()).unwrap_or(0)
    });

    // Create a new temporary tree, with all changes staged
    let mut index = repository.index().map_err(|e| format!("Failed to get repository index: {}", e))?;
    for file_change in &sorted_file_changes {
        match file_change.status {
            FileChangeStatus::ADDED | FileChangeStatus::MODIFIED => {
                index.add_path(&file_change.relative_path)
                    .map_err(|e| format!("Failed to add file to index: {}", e))?;
            },
            FileChangeStatus::DELETED => {
                index.remove_path(&file_change.relative_path)
                    .map_err(|e| format!("Failed to remove file from index: {}", e))?;
            },
        }
    }
    let oid = index.write_tree().map_err(|e| format!("Failed to write tree: {}", e))?;
    let new_tree = repository.find_tree(oid).map_err(|e| format!("Failed to find tree: {}", e))?;

    let head = repository.head().and_then(|head_ref| head_ref.peel_to_tree())
        .map_err(|e| format!("Failed to get HEAD tree: {}", e))?;

    let diff = repository.diff_tree_to_tree(Some(&head), Some(&new_tree), Some(&mut diff_options))
        .map_err(|e| format!("Failed to generate diff: {}", e))?;

    Ok(diff)
}

pub fn git_diff_as_string(repository: &Repository, file_changes: &Vec<FileChange>, max_size: usize) -> Result<String, String> {
    let diff = git_diff(repository, file_changes)?;

    let mut diff_str = String::new();
    diff.print(git2::DiffFormat::Patch, |_, _, line| {
        let line_content = std::str::from_utf8(line.content()).unwrap_or("");
        if diff_str.len() + line_content.len() < max_size {
            diff_str.push(line.origin());
            diff_str.push_str(line_content);
            if diff_str.len() > max_size {
                diff_str.truncate(max_size - 4);
                diff_str.push_str("...\n");
            }
        }
        true
    }).map_err(|e| format!("Failed to print diff: {}", e))?;

    Ok(diff_str)
}

pub fn clone_local_repo_without_checkout(source_dir: &Path, target_dir: &Path) -> Result<Duration, String> {
    let t0 = Instant::now();
    let mut checkout_builder = git2::build::CheckoutBuilder::new();
    checkout_builder.allow_conflicts(true).dry_run();
    let mut repo_builder = RepoBuilder::new();
    repo_builder.bare(false).with_checkout(checkout_builder).clone_local(git2::build::CloneLocal::NoLinks);

    let source_dir_url = url::Url::from_file_path(source_dir)
        .map_err(|_| format!("Failed to convert {} to url.", source_dir.to_string_lossy()))?;
    let repo = repo_builder.clone(source_dir_url.as_str(), target_dir)
        .map_err_with_prefix(format!("Failed to clone repository {}:", source_dir.to_string_lossy()))?;

    repo.set_workdir(&source_dir, false).map_err_with_prefix("Failed to set workdir:")?;

    let head_commit = repo.head()
        .and_then(|head| head.peel_to_commit())
        .or_else(|_| {
            repo.find_branch("master", git2::BranchType::Local)
                .or_else(|_| repo.find_branch("main", git2::BranchType::Local))
                .and_then(|branch| branch.get().peel_to_commit())
                .map_err_to_string()
        }).map_err_with_prefix("Failed to get HEAD commit:")?;

    repo.reset(head_commit.as_object(), git2::ResetType::Mixed, None)
        .map_err_with_prefix("Failed to reset index to HEAD:")?;
    repo.statuses(Some(&mut status_options(true, false, true)))
        .map_err_with_prefix("Failed to get statuses:")?;

    Ok(t0.elapsed())
}

pub fn checkout_head_and_branch_to_commit(repo: &Repository, branch_name: &str, commit_oid: &Oid) -> Result<(), String> {
    let commit = repo.find_commit(commit_oid.clone()).map_err_with_prefix("Failed to find commit:")?;

    let mut branch_ref = repo.find_branch(branch_name, git2::BranchType::Local)
        .map_err_with_prefix("Failed to get branch:")?.into_reference();
    branch_ref.set_target(commit.id(),"Restoring checkpoint")
        .map_err_with_prefix("Failed to update branch reference:")?;

    repo.set_head(&format!("refs/heads/{}", branch_name))
        .map_err_with_prefix("Failed to set HEAD:")?;

    let mut checkout_opts = git2::build::CheckoutBuilder::new();
    checkout_opts.force().update_index(true);
    repo.checkout_head(Some(&mut checkout_opts)).map_err_with_prefix("Failed to checkout HEAD:")?;

    Ok(())
}