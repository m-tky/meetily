//! Synthetic-only portability checks: no models, devices, network or inference.
use qwen_asr::{
    config::QwenConfig,
    int8_sidecar::{self, SidecarLayout, SidecarMmap},
    safetensors::SafetensorsFile,
};
use std::{fs, path::PathBuf, sync::Arc};
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!(
            "meetily-qwen-{}-{unique} 日本語 O'Brien",
            std::process::id()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn model(&self) -> PathBuf {
        let p = self.0.join("model.safetensors");
        let mut header = r#"{"weight":{"dtype":"BF16","shape":[2],"data_offsets":[0,4]}}"#
            .as_bytes()
            .to_vec();
        while header.len() % 8 != 0 {
            header.push(b' ');
        }
        let mut bytes = (header.len() as u64).to_le_bytes().to_vec();
        bytes.extend(header);
        bytes.extend([0x80, 0x3f, 0x00, 0xc0]);
        fs::write(&p, bytes).unwrap();
        p
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn unicode_mapping_and_concurrent_positional_reads() {
    let f = Fixture::new();
    let p = f.model();
    let mapped = Arc::new(SafetensorsFile::open(p.to_str().unwrap()).unwrap());
    let t = mapped.find("weight").unwrap();
    assert_eq!(mapped.get_f32(t).unwrap(), [1.0, -2.0]);
    std::thread::scope(|scope| {
        for offset in [0usize, 2, 0, 2] {
            let mapped = mapped.clone();
            scope.spawn(move || {
                for _ in 0..32 {
                    let mut out = [0u8; 2];
                    assert!(mapped.read_tensor_bytes(
                        mapped.find("weight").unwrap(),
                        offset,
                        &mut out
                    ));
                    assert_eq!(out, if offset == 0 { [0x80, 0x3f] } else { [0, 0xc0] });
                }
            });
        }
    });
    drop(mapped); // Windows mappings/file handles must be released before cleanup.
}
#[test]
fn malformed_headers_are_rejected() {
    let f = Fixture::new();
    let p = f.0.join("bad.safetensors");
    for bytes in [
        vec![],
        vec![0; 7],
        u64::MAX.to_le_bytes().to_vec(),
        vec![1, 0, 0, 0, 0, 0, 0, 0, 0xff],
    ] {
        fs::write(&p, bytes).unwrap();
        assert!(SafetensorsFile::open(p.to_str().unwrap()).is_none());
    }
}
#[cfg(windows)]
#[test]
fn windows_mapping_rejects_out_of_file_offsets() {
    let f = Fixture::new();
    let p = f.model();
    let mut bytes = fs::read(&p).unwrap();
    let s = String::from_utf8(bytes[8..bytes.len() - 4].to_vec())
        .unwrap()
        .replace("[0,4]", "[4,8]");
    bytes[8..8 + s.len()].copy_from_slice(s.as_bytes());
    fs::write(&p, bytes).unwrap();
    assert!(SafetensorsFile::open(p.to_str().unwrap()).is_none());
}
#[test]
fn sidecar_mapping_roundtrip_reopen_and_corruption() {
    let f = Fixture::new();
    f.model();
    let cfg = QwenConfig {
        dec_layers: 1,
        dec_hidden: 4,
        dec_intermediate: 8,
        dec_heads: 2,
        dec_kv_heads: 1,
        dec_head_dim: 2,
        vocab_size: 4,
        ..Default::default()
    };
    let layout = SidecarLayout::compute(&cfg);
    let buffers: Vec<Vec<u8>> = layout
        .lens
        .iter()
        .enumerate()
        .map(|(k, &n)| vec![(k + 1) as u8; n as usize])
        .collect();
    let slices: Vec<&[u8]> = buffers.iter().map(Vec::as_slice).collect();
    let p = f.0.join("decoder INT8.sidecar");
    let path = p.to_str().unwrap();
    let model = f.0.to_str().unwrap();
    assert!(int8_sidecar::write_sidecar(
        path, model, &cfg, &layout, &slices
    ));
    for _ in 0..2 {
        let map = SidecarMmap::open_valid(path, model, &cfg, &layout).unwrap();
        assert_eq!(&*map.i8_buf(0), &vec![1i8; layout.lens[0] as usize]);
        assert_eq!(map.f32_buf(1)[0].to_bits(), u32::from_ne_bytes([2; 4]));
        drop(map);
    }
    assert!(int8_sidecar::write_sidecar(
        path, model, &cfg, &layout, &slices
    ));
    let mut corrupt = fs::read(&p).unwrap();
    corrupt[0] ^= 1;
    fs::write(&p, corrupt).unwrap();
    assert!(SidecarMmap::open_valid(path, model, &cfg, &layout).is_none());
}
#[test]
fn large_weight_buffer_is_zeroed_and_can_be_dropped() {
    let mut v = qwen_asr::kernels::superpage_vec::<f32>(524289);
    assert_eq!(v.len(), 524289);
    assert!(v.iter().all(|&x| x == 0.0));
    v[524288] = 1.0;
    drop(v);
}

#[test]
fn rectangular_matmul_and_transpose_match_reference() {
    // With `blas`, these call cblas_sgemm directly: exercise LP64 argument
    // widths, leading dimensions and both transpose modes, not just DLL load.
    let a = [1., 2., 3., 4., 5., 6.];
    let b = [7., 8., 9., 10., 11., 12.];
    let bt = [7., 9., 11., 8., 10., 12.];
    let expected = [58., 64., 139., 154.];
    let mut out = [0.; 4];
    qwen_asr::kernels::matmul_nn(&mut out, &a, &b, 2, 3, 2);
    assert_eq!(out, expected);
    out.fill(0.);
    qwen_asr::kernels::matmul_t(&mut out, &a, &bt, 2, 3, 2);
    assert_eq!(out, expected);
}
