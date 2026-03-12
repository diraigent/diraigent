use std::path::PathBuf;

pub struct ProtoConfig {
    pub protos: Vec<String>,
    pub includes: Vec<String>,
    pub build_server: bool,
    pub build_client: bool,
}

impl ProtoConfig {
    pub fn new(protos: Vec<String>, includes: Vec<String>) -> Self {
        Self {
            protos,
            includes,
            build_server: true,
            build_client: false,
        }
    }

    pub fn with_server(mut self, enabled: bool) -> Self {
        self.build_server = enabled;
        self
    }

    pub fn with_client(mut self, enabled: bool) -> Self {
        self.build_client = enabled;
        self
    }
}

pub fn compile_protos_with_reflection(
    config: ProtoConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = std::env::var("OUT_DIR")?;
    let descriptor_path = PathBuf::from(&out_dir).join("proto_descriptor.bin");

    let proto_refs: Vec<&str> = config.protos.iter().map(|s| s.as_str()).collect();
    let include_refs: Vec<&str> = config.includes.iter().map(|s| s.as_str()).collect();

    tonic_prost_build::configure()
        .build_server(config.build_server)
        .build_client(config.build_client)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&proto_refs, &include_refs)?;

    println!("cargo:rerun-if-changed=build.rs");
    for proto in &config.protos {
        println!("cargo:rerun-if-changed={}", proto);
    }

    Ok(())
}

pub fn compile_protos(
    protos: &[&str],
    includes: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let config = ProtoConfig::new(
        protos.iter().map(|s| s.to_string()).collect(),
        includes.iter().map(|s| s.to_string()).collect(),
    );
    compile_protos_with_reflection(config)
}
