//! Explicit-consent Windows setup. The frontend cannot supply URLs or executable paths.
//! Preparation never writes provider, language, summary or onboarding preferences.
use super::qwen_provider::QwenSettings;
use serde::{Deserialize, Serialize};
use std::sync::{LazyLock, Mutex};
use tauri::Runtime;
use tokio_util::sync::CancellationToken;

#[derive(Clone, Serialize)]
pub struct SetupInfo {
    available: bool,
    model_dir: Option<String>,
    download_bytes: u64,
    minimum_free_bytes: u64,
}
#[derive(Clone, Serialize)]
pub struct SetupStatus {
    pub id: String,
    pub phase: String,
    pub file: String,
    pub downloaded: u64,
    pub total: u64,
    pub error: Option<String>,
    pub settings: Option<QwenSettings>,
}
struct Run {
    status: SetupStatus,
    cancel: CancellationToken,
}
static RUN: LazyLock<Mutex<Option<Run>>> = LazyLock::new(|| Mutex::new(None));
const MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../nix/qwen-model.json"
));
const CACHE_BYTES: u64 = 1_724_760_064;
// Pinned engine/model cache payload, independently equal on Linux and Windows
// (WINDOWS-AUDIT: cold-cache comparison). Bytes 16..24 contain shard mtimes;
// the initializer additionally validates that identity and the format header.
#[cfg(any(windows, test))]
const CACHE_PAYLOAD_SHA256: &str =
    "96e4ad3f590d30941305c4e7350450247ca9109226b7f2d87661cde73758a3dd";
const HEADROOM: u64 = 512 * 1024 * 1024;
#[derive(Deserialize)]
struct Manifest {
    id: String,
    revision: String,
    files: Vec<ModelFile>,
}
#[derive(Deserialize)]
struct ModelFile {
    file: String,
    bytes: u64,
    sha256: String,
}
fn manifest() -> Result<Manifest, String> {
    let m: Manifest = serde_json::from_str(MANIFEST).map_err(|_| "Invalid bundled manifest")?;
    if m.id != "Qwen/Qwen3-ASR-1.7B"
        || m.revision != "7278e1e70fe206f11671096ffdd38061171dd6e5"
        || m.files.iter().any(|f| {
            f.file.is_empty() || f.file.contains(['/', '\\', ':']) || f.file.starts_with('.')
        })
    {
        return Err("Unexpected bundled model manifest".into());
    }
    Ok(m)
}
fn active(phase: &str) -> bool {
    !matches!(phase, "ready" | "failed" | "cancelled")
}
fn update(id: &str, f: impl FnOnce(&mut SetupStatus)) {
    if let Some(run) = RUN.lock().unwrap().as_mut().filter(|r| r.status.id == id) {
        f(&mut run.status);
    }
}
#[tauri::command]
pub fn qwen_setup_info<R: Runtime>(app: tauri::AppHandle<R>) -> Result<SetupInfo, String> {
    let _ = &app;
    let bytes = manifest()?.files.iter().map(|f| f.bytes).sum();
    #[cfg(all(windows, target_arch = "x86_64", target_env = "msvc"))]
    {
        let settings = super::qwen_provider::bundled_settings(&app)?;
        let available = std::is_x86_feature_detected!("avx2")
            && std::is_x86_feature_detected!("fma")
            && settings.worker_path.is_file()
            && settings.ort_library.is_file()
            && settings
                .worker_path
                .with_file_name("qwen-model-init.exe")
                .is_file();
        return Ok(SetupInfo {
            available,
            model_dir: Some(settings.model_dir.to_string_lossy().into()),
            download_bytes: bytes,
            minimum_free_bytes: bytes + CACHE_BYTES + HEADROOM,
        });
    }
    #[cfg(not(all(windows, target_arch = "x86_64", target_env = "msvc")))]
    Ok(SetupInfo {
        available: false,
        model_dir: None,
        download_bytes: bytes,
        minimum_free_bytes: bytes + CACHE_BYTES + HEADROOM,
    })
}
#[tauri::command]
pub fn qwen_setup_status() -> Option<SetupStatus> {
    RUN.lock().unwrap().as_ref().map(|r| r.status.clone())
}
#[tauri::command]
pub fn qwen_setup_cancel(id: String) -> Result<(), String> {
    let mut runs = RUN.lock().unwrap();
    let run = runs
        .as_mut()
        .filter(|r| r.status.id == id)
        .ok_or("Setup is no longer current")?;
    run.cancel.cancel();
    if run.status.phase == "ready" {
        run.status.phase = "cancelled".into();
        run.status.settings = None;
        run.status.error = Some("Setup cancelled; verified files were kept".into());
    }
    Ok(())
}
#[tauri::command]
pub async fn qwen_setup_start<R: Runtime>(
    app: tauri::AppHandle<R>,
    consent: bool,
) -> Result<String, String> {
    if !consent {
        return Err("Explicit download and local initialization consent is required".into());
    }
    if crate::audio::recording_commands::is_recording().await {
        return Err("Stop recording before Qwen setup".into());
    }
    if !qwen_setup_info(app.clone())?.available {
        return Err(
            "Automatic setup requires the Windows x64 installer and an AVX2/FMA CPU".into(),
        );
    }
    #[cfg(windows)]
    {
        let settings = super::qwen_provider::bundled_settings(&app)?;
        let id = uuid::Uuid::new_v4().to_string();
        let cancel = CancellationToken::new();
        {
            let mut runs = RUN.lock().unwrap();
            if runs.as_ref().is_some_and(|r| active(&r.status.phase)) {
                return Err("Qwen setup is already running".into());
            }
            *runs = Some(Run {
                status: SetupStatus {
                    id: id.clone(),
                    phase: "checking".into(),
                    file: String::new(),
                    downloaded: 0,
                    total: manifest()?.files.iter().map(|f| f.bytes).sum(),
                    error: None,
                    settings: None,
                },
                cancel: cancel.clone(),
            });
        }
        let job_id = id.clone();
        tauri::async_runtime::spawn(async move {
            let result = prepare(&settings, &cancel, &job_id).await;
            update(&job_id, |s| {
                if cancel.is_cancelled() {
                    s.phase = "cancelled".into();
                    s.error = Some("Setup cancelled; verified files were kept".into());
                } else {
                    match result {
                        Ok(()) => {
                            s.phase = "ready".into();
                            s.settings = Some(settings);
                        }
                        Err(e) => {
                            s.phase = "failed".into();
                            s.error = Some(e);
                        }
                    }
                }
            });
        });
        return Ok(id);
    }
    #[cfg(not(windows))]
    Err("Automatic Qwen setup is only available in the Windows installer".into())
}

#[cfg(any(windows, test))]
mod preparation {
    use super::*;
    use futures_util::StreamExt;
    use sha2::{Digest, Sha256};
    use std::{
        fs,
        io::{Read, Seek, SeekFrom, Write},
        path::Path,
        process::Stdio,
        time::Duration,
    };

    pub(super) fn check_cancel(cancel: &CancellationToken) -> Result<(), String> {
        if cancel.is_cancelled() {
            Err("Setup cancelled; verified files were kept. Retry requires consent again.".into())
        } else {
            Ok(())
        }
    }
    // Reject links/reparse points in every existing ancestor, not just the leaf.
    pub(super) fn no_links(path: &Path) -> Result<(), String> {
        for ancestor in path.ancestors() {
            match fs::symlink_metadata(ancestor) {
                Ok(m) => {
                    #[cfg(windows)]
                    let linked = {
                        use std::os::windows::fs::MetadataExt;
                        m.file_attributes() & 0x400 != 0
                    };
                    #[cfg(not(windows))]
                    let linked = m.file_type().is_symlink();
                    if linked {
                        return Err("Linked/reparse-point setup paths are not supported".into());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(format!("Cannot inspect setup path: {e}")),
            }
        }
        Ok(())
    }
    pub(super) fn sha256(path: &Path, cancel: &CancellationToken) -> Result<String, String> {
        sha256_from(path, cancel, 0)
    }
    fn sha256_from(path: &Path, cancel: &CancellationToken, offset: u64) -> Result<String, String> {
        let mut f = fs::File::open(path).map_err(|e| format!("Cannot read model file: {e}"))?;
        f.seek(SeekFrom::Start(offset)).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buf = vec![0; 1024 * 1024];
        loop {
            check_cancel(cancel)?;
            let n = f
                .read(&mut buf)
                .map_err(|e| format!("Cannot hash model file: {e}"))?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    pub(super) fn verify(
        path: &Path,
        row: &ModelFile,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        no_links(path)?;
        let meta = fs::metadata(path).map_err(|e| format!("Cannot inspect {}: {e}", row.file))?;
        if !meta.is_file() || meta.len() != row.bytes || sha256(path, cancel)? != row.sha256 {
            return Err(format!(
                "Existing model file differs; not overwritten: {}",
                row.file
            ));
        }
        Ok(())
    }
    pub(super) async fn download(
        client: &reqwest::Client,
        url: &str,
        root: &Path,
        row: &ModelFile,
        cancel: &CancellationToken,
        id: &str,
    ) -> Result<(), String> {
        let mut temporary = tempfile::Builder::new()
            .prefix(".download-")
            .tempfile_in(root)
            .map_err(|e| e.to_string())?;
        let response = tokio::select! {
            _ = cancel.cancelled() => return check_cancel(cancel),
            r = client.get(url).send() => r.map_err(|_| format!("Download failed: {}. Check the network and retry.", row.file))?.error_for_status().map_err(|_| format!("Model server rejected {}", row.file))?,
        };
        let mut stream = response.bytes_stream();
        let mut received = 0u64;
        let mut hash = Sha256::new();
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => return check_cancel(cancel),
                chunk = stream.next() => chunk,
            };
            let Some(chunk) = next else {
                break;
            };
            let chunk = chunk.map_err(|_| {
                format!(
                    "Download interrupted: {}. Verified files were kept.",
                    row.file
                )
            })?;
            received += chunk.len() as u64;
            if received > row.bytes {
                return Err(format!(
                    "Model download exceeds expected size: {}",
                    row.file
                ));
            }
            hash.update(&chunk);
            temporary
                .write_all(&chunk)
                .map_err(|e| format!("Cannot write model download: {e}"))?;
            update(id, |s| s.downloaded += chunk.len() as u64);
        }
        if received != row.bytes || format!("{:x}", hash.finalize()) != row.sha256 {
            return Err(format!("Downloaded model checksum mismatch: {}", row.file));
        }
        check_cancel(cancel)?;
        temporary.as_file().sync_all().map_err(|e| e.to_string())?;
        temporary
            .persist_noclobber(root.join(&row.file))
            .map_err(|_| "Model destination appeared during setup; not overwritten".to_string())?;
        Ok(())
    }
    pub(super) async fn initialize(
        settings: &QwenSettings,
        cancel: &CancellationToken,
    ) -> Result<(), String> {
        // Same-volume hardlinks preserve weight mtimes used by the cache identity.
        // Quantization happens in isolation: even a stale cache cannot be overwritten.
        let staging = tempfile::Builder::new()
            .prefix(".meetily-setup-")
            .tempdir_in(&settings.model_dir)
            .map_err(|e| e.to_string())?;
        for row in manifest()?.files {
            fs::hard_link(
                settings.model_dir.join(&row.file),
                staging.path().join(row.file),
            )
            .map_err(|e| format!("Cannot stage model on this filesystem: {e}"))?;
        }
        let cache = settings.model_dir.join("qwen-asr-int8.sidecar");
        no_links(&cache)?;
        if cache.exists() {
            verify_cache(&cache, cancel)?;
            fs::hard_link(&cache, staging.path().join("qwen-asr-int8.sidecar"))
                .map_err(|e| e.to_string())?;
        }
        let mut command = tokio::process::Command::new(
            settings.worker_path.with_file_name("qwen-model-init.exe"),
        );
        command
            .arg(staging.path())
            .env("OPENBLAS_NUM_THREADS", "6")
            .env("OMP_NUM_THREADS", "6")
            .env("MKL_NUM_THREADS", "6")
            .env("HF_HUB_DISABLE_TELEMETRY", "1")
            .env("DO_NOT_TRACK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true);
        #[cfg(windows)]
        command.creation_flags(0x08000000);
        let mut child = command
            .spawn()
            .map_err(|e| format!("Cannot launch bundled initializer: {e}"))?;
        #[cfg(windows)]
        let _job = super::windows_job::Job::attach(
            child
                .raw_handle()
                .ok_or("Initializer exited before process guarding")?,
        )?;
        let result = tokio::select! {
            _ = cancel.cancelled() => { let _ = child.kill().await; return check_cancel(cancel); }
            _ = tokio::time::sleep(Duration::from_secs(1800)) => { let _ = child.kill().await; return Err("Local initialization timed out; verified files were kept".into()); }
            result = child.wait() => result.map_err(|e| e.to_string())?,
        };
        if !result.success() {
            return Err(format!("Local initialization failed ({result}). Existing files were not overwritten; check memory and any existing cache."));
        }
        let prepared = staging.path().join("qwen-asr-int8.sidecar");
        if fs::metadata(&prepared)
            .map_err(|_| "Initializer did not create its cache")?
            .len()
            != CACHE_BYTES
        {
            return Err("Unexpected local cache size".into());
        }
        verify_cache(&prepared, cancel)?;
        if cache.exists() {
            if sha256(&cache, cancel)? != sha256(&prepared, cancel)? {
                return Err("Existing INT8 cache differs; not overwritten".into());
            }
        } else {
            // Hardlink is atomic/no-clobber on the same volume; staging cleanup removes only its link.
            fs::hard_link(&prepared, &cache)
                .map_err(|e| format!("Cannot publish local cache without overwriting: {e}"))?;
        }
        Ok(())
    }
    fn verify_cache(path: &Path, cancel: &CancellationToken) -> Result<(), String> {
        verify_cache_payload(path, cancel, CACHE_BYTES, CACHE_PAYLOAD_SHA256)
    }
    pub(super) fn verify_cache_payload(
        path: &Path,
        cancel: &CancellationToken,
        bytes: u64,
        digest: &str,
    ) -> Result<(), String> {
        no_links(path)?;
        if fs::metadata(path).map_err(|e| e.to_string())?.len() != bytes
            || sha256_from(path, cancel, 24)? != digest
        {
            return Err(
                "INT8 cache differs from the pinned model/engine payload; not overwritten".into(),
            );
        }
        Ok(())
    }
    pub(super) fn check_weight_set(root: &Path, m: &Manifest) -> Result<(), String> {
        for entry in fs::read_dir(root).map_err(|e| e.to_string())? {
            let name = entry
                .map_err(|e| e.to_string())?
                .file_name()
                .to_string_lossy()
                .into_owned();
            let lower = name.to_ascii_lowercase();
            if (lower == "model.safetensors"
                || (lower.starts_with("model-") && lower.ends_with(".safetensors")))
                && !m.files.iter().any(|row| row.file == name)
            {
                return Err(format!("Unexpected model weights in setup directory; preserved, not overwritten: {name}"));
            }
        }
        Ok(())
    }
    pub(super) async fn prepare(
        settings: &QwenSettings,
        cancel: &CancellationToken,
        id: &str,
    ) -> Result<(), String> {
        no_links(&settings.model_dir)?;
        fs::create_dir_all(&settings.model_dir).map_err(|e| e.to_string())?;
        let lock_path = settings.model_dir.join(".meetily-preparation.lock");
        no_links(&lock_path)?;
        let lease = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&lease).map_err(|_| {
            "Another Qwen preparation is running. Wait for it to finish.".to_string()
        })?;
        let m = manifest()?;
        check_weight_set(&settings.model_dir, &m)?;
        let mut missing = 0;
        for row in &m.files {
            check_cancel(cancel)?;
            let path = settings.model_dir.join(&row.file);
            no_links(&path)?;
            update(id, |s| {
                s.phase = "checking".into();
                s.file = row.file.clone();
            });
            if path.exists() {
                verify(&path, row, cancel)?;
                update(id, |s| s.downloaded += row.bytes);
            } else {
                missing += row.bytes;
            }
        }
        // Include an isolated replacement cache even when an old cache exists.
        if fs2::available_space(&settings.model_dir).map_err(|e| e.to_string())?
            < missing + CACHE_BYTES + HEADROOM
        {
            return Err(
                "Not enough free disk space for the remaining files and local INT8 initialization"
                    .into(),
            );
        }
        let client = reqwest::Client::builder()
            .https_only(true)
            .connect_timeout(Duration::from_secs(30))
            .timeout(Duration::from_secs(1800))
            .build()
            .map_err(|e| e.to_string())?;
        for row in &m.files {
            check_cancel(cancel)?;
            if settings.model_dir.join(&row.file).exists() {
                continue;
            }
            update(id, |s| {
                s.phase = "downloading".into();
                s.file = row.file.clone();
            });
            let url = format!(
                "https://huggingface.co/{}/resolve/{}/{}",
                m.id, m.revision, row.file
            );
            download(&client, &url, &settings.model_dir, row, cancel, id).await?;
        }
        check_cancel(cancel)?;
        update(id, |s| {
            s.phase = "initializing".into();
            s.file.clear();
        });
        initialize(settings, cancel).await?;
        check_cancel(cancel)?;
        check_weight_set(&settings.model_dir, &m)?;
        update(id, |s| s.phase = "validating".into());
        let provider = super::super::qwen_provider::QwenProvider::new(
            settings.clone(),
            settings.model_dir.join(".setup-audio"),
        )?;
        let result = tokio::select! { _ = cancel.cancelled() => check_cancel(cancel), r = provider.connect() => r };
        provider.shutdown().await;
        result?;
        check_cancel(cancel)
    }
}
#[cfg(windows)]
use preparation::prepare;

#[cfg(windows)]
mod windows_job {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::JobObjects::*,
    };
    // Closing this handle (including OS process teardown) kills the owned initializer.
    pub(super) struct Job(HANDLE);
    unsafe impl Send for Job {}
    impl Job {
        pub(super) fn attach(process: HANDLE) -> Result<Self, String> {
            unsafe {
                let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if handle.is_null() {
                    return Err("Cannot create initializer process guard".into());
                }
                let job = Self(handle);
                let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    handle,
                    JobObjectExtendedLimitInformation,
                    &limits as *const _ as *const _,
                    std::mem::size_of_val(&limits) as u32,
                ) == 0
                {
                    return Err("Cannot configure initializer process guard".into());
                }
                // Borrow the owned Child's handle, never reopen by PID (which
                // could race exit/reuse). The Child retains handle ownership.
                let assigned = AssignProcessToJobObject(handle, process);
                if assigned == 0 {
                    return Err("Cannot attach initializer process guard".into());
                }
                Ok(job)
            }
        }
    }
    impl Drop for Job {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{preparation::*, *};
    use sha2::{Digest, Sha256};
    #[cfg(windows)]
    #[tokio::test]
    async fn windows_job_close_terminates_only_the_owned_child() {
        use std::{path::PathBuf, process::Stdio, time::Duration};
        use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader};
        let powershell = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let mut child = tokio::process::Command::new(powershell)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::Out.WriteLine('READY'); [Threading.Thread]::Sleep(60000); [Console]::Out.WriteLine('UNEXPECTED_NATURAL_EXIT')",
            ])
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let job = super::windows_job::Job::attach(child.raw_handle().unwrap()).unwrap();
        let mut stdout = BufReader::new(child.stdout.take().unwrap());
        let mut ready = String::new();
        tokio::time::timeout(Duration::from_secs(15), stdout.read_line(&mut ready))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ready.trim(), "READY");
        assert!(
            tokio::time::timeout(Duration::from_millis(250), child.wait())
                .await
                .is_err(),
            "The child must remain alive while the job handle is open"
        );
        drop(job);
        // Windows job-close termination can return exit code 0. Prove that
        // the known-live 60s sleeper exits within 5s, without its natural-exit
        // marker, rather than treating an exit-code convention as liveness.
        tokio::time::timeout(Duration::from_secs(5), child.wait())
            .await
            .unwrap()
            .unwrap();
        let mut trailing = String::new();
        tokio::time::timeout(Duration::from_secs(5), stdout.read_to_string(&mut trailing))
            .await
            .unwrap()
            .unwrap();
        assert!(
            trailing.trim().is_empty(),
            "The child must not complete its natural sleep: {trailing}"
        );
    }
    #[test]
    fn manifest_is_fixed_and_downloads_are_not_a_ui_parameter() {
        let m = manifest().unwrap();
        assert_eq!(m.files.len(), 9);
        assert!(m.files.iter().all(|r| r.sha256.len() == 64 && r.bytes > 0));
        assert!(
            active("checking")
                && active("validating")
                && !active("ready")
                && !active("failed")
                && !active("cancelled")
        );
    }
    #[test]
    fn late_cancel_revokes_ready_result_and_rejects_stale_ids() {
        let id = "cancel-test";
        *RUN.lock().unwrap() = Some(Run {
            status: SetupStatus {
                id: id.into(),
                phase: "ready".into(),
                file: String::new(),
                downloaded: 0,
                total: 0,
                error: None,
                settings: Some(QwenSettings {
                    worker_path: "worker".into(),
                    model_dir: "model".into(),
                    ort_library: "ort".into(),
                }),
            },
            cancel: CancellationToken::new(),
        });
        assert!(qwen_setup_cancel("other".into()).is_err());
        assert_eq!(qwen_setup_status().unwrap().phase, "ready");
        qwen_setup_cancel(id.into()).unwrap();
        let state = qwen_setup_status().unwrap();
        assert_eq!(state.phase, "cancelled");
        assert!(state.settings.is_none());
        *RUN.lock().unwrap() = None;
    }
    #[test]
    fn existing_file_mismatch_and_cancel_do_not_modify_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("weights");
        std::fs::write(&path, b"original").unwrap();
        let row = ModelFile {
            file: "weights".into(),
            bytes: 8,
            sha256: format!("{:x}", Sha256::digest(b"original")),
        };
        let cancel = CancellationToken::new();
        verify(&path, &row, &cancel).unwrap();
        assert!(verify(&path, &ModelFile { bytes: 9, ..row }, &cancel)
            .unwrap_err()
            .contains("not overwritten"));
        cancel.cancel();
        assert!(sha256(&path, &cancel).is_err());
        assert_eq!(std::fs::read(path).unwrap(), b"original");
    }
    async fn server(body: &'static [u8]) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0u8; 2048];
            let _ = socket.read(&mut request).await;
            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            socket.write_all(header.as_bytes()).await.unwrap();
            socket.write_all(body).await.unwrap();
        });
        (format!("http://{address}/fixture"), task)
    }
    #[tokio::test]
    async fn download_checks_bytes_hash_and_never_clobbers() {
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let row = ModelFile {
            file: "fixture".into(),
            bytes: 4,
            sha256: format!("{:x}", Sha256::digest(b"test")),
        };
        for body in [b"bad!".as_slice(), b"short".as_slice(), b"x".as_slice()] {
            let (url, server) = server(body).await;
            assert!(
                download(&client, &url, dir.path(), &row, &cancel, "fixture")
                    .await
                    .is_err()
            );
            server.await.unwrap();
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
        }
        let (url, task) = server(b"test").await;
        download(&client, &url, dir.path(), &row, &cancel, "fixture")
            .await
            .unwrap();
        task.await.unwrap();
        assert_eq!(std::fs::read(dir.path().join("fixture")).unwrap(), b"test");
        std::fs::write(dir.path().join("fixture"), b"kept").unwrap();
        let (url, task) = server(b"test").await;
        assert!(
            download(&client, &url, dir.path(), &row, &cancel, "fixture")
                .await
                .unwrap_err()
                .contains("not overwritten")
        );
        task.await.unwrap();
        assert_eq!(std::fs::read(dir.path().join("fixture")).unwrap(), b"kept");
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
    #[tokio::test]
    async fn cancel_stalled_download_removes_partial_file() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let dir = tempfile::tempdir().unwrap();
        let cancel = CancellationToken::new();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/fixture", listener.local_addr().unwrap());
        let trigger = cancel.clone();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            let _ = socket.read(&mut request).await;
            socket
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nx")
                .await
                .unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(30)).await;
            trigger.cancel();
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        });
        let row = ModelFile {
            file: "fixture".into(),
            bytes: 100,
            sha256: "unused".into(),
        };
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let error = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            download(&client, &url, dir.path(), &row, &cancel, "fixture"),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error.contains("cancelled"));
        task.await.unwrap();
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    }
    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_reaps_initializer_and_removes_only_its_staging() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::Builder::new()
            .prefix("qwen setup 日本語-")
            .tempdir()
            .unwrap();
        let model = dir.path().join("model");
        std::fs::create_dir(&model).unwrap();
        for row in manifest().unwrap().files {
            std::fs::write(model.join(row.file), b"fixture").unwrap();
        }
        let pid_file = dir.path().join("initializer.pid");
        let initializer = dir.path().join("qwen-model-init.exe");
        std::fs::write(
            &initializer,
            format!(
                "#!/bin/sh\nprintf '%s' \"$$\" > '{}'\nexec sleep 60\n",
                pid_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&initializer, std::fs::Permissions::from_mode(0o700)).unwrap();
        let settings = QwenSettings {
            worker_path: dir.path().join("worker.exe"),
            model_dir: model.clone(),
            ort_library: dir.path().join("ort.dll"),
        };
        let cancel = CancellationToken::new();
        let trigger = cancel.clone();
        let task = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            trigger.cancel();
        });
        let error = initialize(&settings, &cancel).await.unwrap_err();
        task.await.unwrap();
        assert!(error.contains("cancelled"));
        let pid = std::fs::read_to_string(pid_file).unwrap();
        assert!(!std::path::Path::new("/proc").join(pid).exists());
        assert_eq!(std::fs::read_dir(model).unwrap().count(), 9);
    }
    #[test]
    fn unexpected_active_weights_are_refused_but_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let m = manifest().unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"keep").unwrap();
        check_weight_set(dir.path(), &m).unwrap();
        let extra = dir.path().join("model.safetensors");
        std::fs::write(&extra, b"keep").unwrap();
        assert!(check_weight_set(dir.path(), &m).is_err());
        assert_eq!(std::fs::read(extra).unwrap(), b"keep");
        assert_eq!(
            std::fs::read(dir.path().join("notes.txt")).unwrap(),
            b"keep"
        );
    }
    #[test]
    fn cache_payload_rejects_corruption_without_replacing_existing_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache");
        let mut cache = vec![0; 24];
        cache.extend_from_slice(b"payload");
        let cancel = CancellationToken::new();
        let digest = format!("{:x}", Sha256::digest(b"payload"));
        std::fs::write(&path, &cache).unwrap();
        verify_cache_payload(&path, &cancel, 31, &digest).unwrap();
        cache[16] = 42; // Timestamp-derived identity is validated by the initializer.
        std::fs::write(&path, &cache).unwrap();
        verify_cache_payload(&path, &cancel, 31, &digest).unwrap();
        cache[25] ^= 1;
        std::fs::write(&path, &cache).unwrap();
        assert!(verify_cache_payload(&path, &cancel, 31, &digest)
            .unwrap_err()
            .contains("not overwritten"));
        assert_eq!(std::fs::read(&path).unwrap(), cache);
    }
    #[cfg(unix)]
    #[test]
    fn linked_ancestor_is_rejected_before_directory_creation() {
        let dir = tempfile::tempdir().unwrap();
        let linked = dir.path().join("linked");
        std::os::unix::fs::symlink(dir.path(), &linked).unwrap();
        assert!(no_links(&linked.join("new/model")).is_err());
        assert!(!dir.path().join("new").exists());
    }
}
