//! Approved comparison only: official-current ONNX contract at a pinned revision.
//! Rust port of OnnxWrapper 16k path in utils_vad.py, MIT/Silero Team.
//! 512 new samples +64 context, state[2,1,128], scalar i64 sr. Never use v4 adapter.
use anyhow::{bail, Result};
use ort::{session::Session, value::Tensor};
use qwen_speech_worker::{read_wav, vad_gain};
use serde_json::json;

fn frame_input(context: &[f32], chunk: &[f32], gain: f32) -> Result<Vec<f32>> {
    if context.len() != 64
        || chunk.is_empty()
        || chunk.len() > 512
        || !gain.is_finite()
        || gain < 1.0
    {
        bail!("invalid_frame_contract");
    }
    let mut input = context.to_vec();
    input.extend(chunk.iter().map(|x| x * gain));
    input.resize(576, 0.0);
    Ok(input)
}

fn main() -> Result<()> {
    ort::init_from(std::env::var("ORT_DYLIB_PATH")?).commit()?;
    let args: Vec<_> = std::env::args().collect();
    if args.len() < 3 {
        bail!("usage: current_vad_probe PINNED_MODEL.onnx WAV...");
    }
    let mut model = Session::builder()?
        .with_intra_threads(1)?
        .with_inter_threads(1)?
        .commit_from_file(&args[1])?;
    let mut inputs: Vec<_> = model.inputs.iter().map(|i| i.name.as_str()).collect();
    inputs.sort_unstable();
    let mut outputs: Vec<_> = model.outputs.iter().map(|o| o.name.as_str()).collect();
    outputs.sort_unstable();
    println!(
        "{}",
        json!({"event":"model_schema","inputs":model.inputs.iter().map(|i|json!({"name":i.name,"type":format!("{:?}",i.input_type)})).collect::<Vec<_>>(),
        "outputs":model.outputs.iter().map(|o|json!({"name":o.name,"type":format!("{:?}",o.output_type)})).collect::<Vec<_>>(),"onnx_intra_threads":1,"onnx_inter_threads":1})
    );
    if inputs != ["input", "sr", "state"] || outputs != ["output", "stateN"] {
        bail!("unexpected_model_contract");
    }
    for path in &args[2..] {
        let samples = read_wav(std::path::Path::new(path))?;
        let peak = samples.iter().fold(0.0f32, |p, x| p.max(x.abs()));
        let peak100 = if peak > 0.0 {
            (0.1 / peak).clamp(1.0, 100.0)
        } else {
            1.0
        };
        for (policy, gain) in [
            ("none", 1.0),
            ("peak20", vad_gain(&samples)),
            ("peak100", peak100),
        ] {
            let mut state = vec![0.0f32; 256];
            let mut context = vec![0.0f32; 64];
            let mut probabilities = Vec::new();
            for chunk in samples.chunks(512) {
                let input = frame_input(&context, chunk, gain)?;
                let next_context = input[512..576].to_vec();
                let result = model.run(ort::inputs![
                    "input" => Tensor::from_array(([1usize, 576], input.into_boxed_slice()))?,
                    "state" => Tensor::from_array(([2usize, 1, 128], state.into_boxed_slice()))?,
                    "sr" => Tensor::from_array((Vec::<usize>::new(), vec![16000_i64].into_boxed_slice()))?
                ])?;
                let (shape, data) = result["output"].try_extract_tensor::<f32>()?;
                if shape.as_ref() != [1, 1]
                    || data.len() != 1
                    || !data[0].is_finite()
                    || !(0.0..=1.0).contains(&data[0])
                {
                    bail!("invalid_probability_contract");
                }
                probabilities.push(data[0]);
                let (shape, data) = result["stateN"].try_extract_tensor::<f32>()?;
                if shape.as_ref() != [2, 1, 128]
                    || data.len() != 256
                    || data.iter().any(|x| !x.is_finite())
                {
                    bail!("invalid_state_contract");
                }
                state = data.to_vec();
                context = next_context;
            }
            println!(
                "{}",
                json!({"file":path,"policy":policy,"gain":gain,"real_samples":samples.len(),
                "new_frame_samples":512,"context_samples":64,"onnx_input_samples":576,"fresh_state_per_input":true,
                "probabilities":probabilities})
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_frame_context_and_padding() {
        let context = vec![0.2; 64];
        let frame = frame_input(&context, &[0.1; 512], 2.0).unwrap();
        assert_eq!(frame.len(), 576);
        assert_eq!(&frame[..64], &context);
        assert!(frame[64..].iter().all(|x| *x == 0.2));
        let tail = frame_input(&context, &[0.1], 2.0).unwrap();
        assert_eq!(tail[64], 0.2);
        assert!(tail[65..].iter().all(|x| *x == 0.0));
        assert_eq!(&tail[512..], &[0.0; 64]);
    }
    #[test]
    fn incompatible_frames_reject() {
        assert!(frame_input(&[0.0; 63], &[0.0; 512], 1.0).is_err());
        assert!(frame_input(&[0.0; 64], &[0.0; 513], 1.0).is_err());
        assert!(frame_input(&[0.0; 64], &[], 1.0).is_err());
    }
}
