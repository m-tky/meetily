//! Synthetic protocol fixture only. No models, device capture or network.
use serde_json::json;
use std::io::{BufRead, Write};
fn main() {
    let root = std::path::PathBuf::from(std::env::args_os().nth(1).expect("fixture root"));
    std::fs::write(root.join("pid"), std::process::id().to_string()).unwrap();
    println!(
        "{}",
        json!({"event":"ready","protocol":1,"threads":6,"model":"Qwen3-ASR-1.7B","precision":"decoder-INT8-encoder-prefill-F32","recognition_context":"full_input","vad_only":false,"max_audio_seconds":30,"vad_frame_samples":512,"audio_formats":["float32"]})
    );
    std::io::stdout().flush().unwrap();
    for line in std::io::stdin().lock().lines() {
        let request: serde_json::Value = serde_json::from_str(&line.unwrap()).unwrap();
        let behavior = std::fs::read_to_string(root.join("behavior")).unwrap();
        match behavior.trim() {
            "hang" => {
                std::thread::sleep(std::time::Duration::from_secs(30));
                continue;
            }
            "crash" => std::process::exit(7),
            "healthy" => {}
            _ => panic!("unknown fixture behavior"),
        }
        let bytes = std::fs::read(request["wav"].as_str().unwrap()).unwrap();
        assert_eq!(&bytes[20..24], &[3, 0, 1, 0]);
        assert_eq!(&bytes[24..28], &16000u32.to_le_bytes());
        let samples = (bytes.len() - 58) / 4;
        assert!(samples > 0 && samples <= 480000);
        assert!(bytes[58..]
            .chunks_exact(4)
            .all(|b| b == &1e-6f32.to_le_bytes()));
        println!(
            "{}",
            json!({"id":request["id"],"confirmed":false,"text":format!("samples={samples}"),"asr_calls":1,"status":"recognized","recognition_context":"full_input"})
        );
        std::io::stdout().flush().unwrap();
    }
}
