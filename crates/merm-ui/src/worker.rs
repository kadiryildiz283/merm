use merm_core::advisor::{AdviceProposal, Advisor, CheckReport};
use merm_core::manifest::ProjectManifest;
use merm_core::node_runner::{ExecutionResult, NodeRunner};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::thread;

pub enum WorkerTask {
    Check {
        manifest: ProjectManifest,
        diagram_source: String,
    },
    Advice {
        manifest: ProjectManifest,
        diagram_source: String,
        prompt: String,
    },
    Ok {
        manifest: ProjectManifest,
        proposal: AdviceProposal,
        diagram_source: String,
    },
    Ai {
        manifest: ProjectManifest,
        diagram_source: String,
        prompt: String,
    },
    Test {
        project_root: PathBuf,
        node_id: String,
        file_path: String,
        entrypoint: Option<String>,
        input: String,
    },
}

pub enum WorkerResult {
    CheckFinished(Result<CheckReport, String>),
    AdviceFinished(Result<AdviceProposal, String>),
    OkFinished(Result<(String, Option<String>), String>),
    AiFinished(Result<(String, Option<String>), String>),
    TestFinished {
        node_id: String,
        result: Result<ExecutionResult, String>,
    },
}

pub struct AsyncWorker {
    task_tx: Sender<WorkerTask>,
}

impl AsyncWorker {
    pub fn spawn(result_tx: Sender<WorkerResult>) -> Self {
        let (task_tx, task_rx) = channel::<WorkerTask>();

        thread::Builder::new()
            .name("merm-cmd-worker".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("Failed to build Tokio worker runtime");

                while let Ok(task) = task_rx.recv() {
                    match task {
                        WorkerTask::Check {
                            manifest,
                            diagram_source,
                        } => {
                            let res = rt
                                .block_on(Advisor::run_check(&manifest, &diagram_source))
                                .map_err(|e| e.to_string());
                            let _ = result_tx.send(WorkerResult::CheckFinished(res));
                        }
                        WorkerTask::Advice {
                            manifest,
                            diagram_source,
                            prompt,
                        } => {
                            let res = rt
                                .block_on(Advisor::request_advice(
                                    &manifest,
                                    &diagram_source,
                                    &prompt,
                                ))
                                .map_err(|e| e.to_string());
                            let _ = result_tx.send(WorkerResult::AdviceFinished(res));
                        }
                        WorkerTask::Ok {
                            mut manifest,
                            proposal,
                            diagram_source,
                        } => {
                            let res =
                                Advisor::apply_advice(&mut manifest, &proposal, &diagram_source)
                                    .map(|msg| (msg, proposal.suggested_diagram.clone()))
                                    .map_err(|e| e.to_string());
                            let _ = result_tx.send(WorkerResult::OkFinished(res));
                        }
                        WorkerTask::Ai {
                            mut manifest,
                            diagram_source,
                            prompt,
                        } => {
                            let res = rt
                                .block_on(Advisor::execute_ai(
                                    &mut manifest,
                                    &diagram_source,
                                    &prompt,
                                ))
                                .map_err(|e| e.to_string());
                            let _ = result_tx.send(WorkerResult::AiFinished(res));
                        }
                        WorkerTask::Test {
                            project_root,
                            node_id,
                            file_path,
                            entrypoint,
                            input,
                        } => {
                            let res = NodeRunner::execute_node(
                                &project_root,
                                &node_id,
                                &file_path,
                                entrypoint.as_deref(),
                                &input,
                            )
                            .map_err(|e| e.to_string());
                            let _ = result_tx.send(WorkerResult::TestFinished {
                                node_id,
                                result: res,
                            });
                        }
                    }
                }
            })
            .expect("Failed to spawn command worker thread");

        Self { task_tx }
    }

    pub fn dispatch(&self, task: WorkerTask) -> Result<(), String> {
        self.task_tx
            .send(task)
            .map_err(|e| format!("Worker dispatch failed: {e}"))
    }
}
