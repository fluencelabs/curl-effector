#![feature(try_blocks)]
#![feature(assert_matches)]
#![allow(improper_ctypes)]
#![allow(non_snake_case)]

mod import;
mod utils;

use curl_effector_types::*;
use eyre::{eyre, Result};
use marine_rs_sdk::marine;
use marine_rs_sdk::module_manifest;
use marine_rs_sdk::WasmLoggerBuilder;

use crate::import::curl;
use crate::utils::{check_url, inject_vault};

module_manifest!();

const CONNECT_TIMEOUT: usize = 4;

pub fn main() {
    WasmLoggerBuilder::new()
        .with_log_level(log::LevelFilter::Debug)
        .build()
        .unwrap();
}

fn run_curl(mut cmd: Vec<String>) -> Result<String> {
    let mut default_arguments = vec![
        String::from("--connect-timeout"),
        format!("{}", CONNECT_TIMEOUT),
        String::from("--no-progress-meter"),
        String::from("--retry"),
        String::from("0"),
    ];
    cmd.append(&mut default_arguments);

    log::debug!("curl arguments: {:?}", cmd);
    let result = curl(cmd.clone());
    log::debug!("curl result: {:?}", result.stringify());

    result
        .into_std()
        .ok_or(eyre!("stdout or stderr contains non valid UTF8 string"))?
        .map_err(|e| eyre!("curl cli call failed \n{:?}: {}", cmd.join(" "), e))
}

fn format_header_args(headers: &[HttpHeader]) -> Vec<String> {
    let mut result = Vec::new();
    for header in headers {
        result.push("-H".to_string());
        result.push(format!("{}: {}", header.name, header.value))
    }
    result
}

//
// curl <url> -X POST
//      --data @<data_vault_path>
//      -H <headers[0]> -H <headers[1]> -H ...
//      -o <output_vault_path>
//      --connect-timeout CONNECT_TIMEOUT
//      --no-progress-meter
//      --retry 0
#[marine]
pub fn curl_post(
    request: CurlRequest,
    data_vault_path: &str,
    output_vault_path: &str,
) -> CurlResult {
    curl_post_impl(request, data_vault_path, output_vault_path).into()
}

fn curl_post_impl(
    request: CurlRequest,
    data_vault_path: &str,
    output_vault_path: &str,
) -> Result<String> {
    let url = check_url(request.url)?;
    let data_vault_path = inject_vault(data_vault_path)?;
    let output_vault_path = inject_vault(output_vault_path)?;
    let mut args = vec![
        String::from(url),
        String::from("-X"),
        String::from("POST"),
        String::from("--data"),
        format!("@{}", data_vault_path),
        String::from("-o"),
        output_vault_path,
    ];
    let mut headers = format_header_args(&request.headers);
    args.append(&mut headers);
    run_curl(args).map(|res| res.trim().to_string())
}

// curl <url> -X GET
//      -H <headers[0]> -H <headers[1]> -H ...
//      -o <output_vault_path>
//      --connect-timeout <connect-timeout>
//      --no-progress-meter
//      --retry 0
#[marine]
pub fn curl_get(request: CurlRequest, output_vault_path: &str) -> CurlResult {
    curl_get_impl(request, output_vault_path).into()
}
pub fn curl_get_impl(request: CurlRequest, output_vault_path: &str) -> Result<String> {
    let url = check_url(request.url)?;
    let output_vault_path = inject_vault(output_vault_path)?;
    let mut args = vec![
        String::from(url),
        String::from("-X"),
        String::from("GET"),
        String::from("-o"),
        output_vault_path,
    ];
    let mut headers = format_header_args(&request.headers);
    args.append(&mut headers);
    run_curl(args).map(|res| res.trim().to_string())
}

#[test_env_helpers::before_each]
#[test_env_helpers::after_each]
#[test_env_helpers::after_all]
#[cfg(test)]
mod tests {
    use marine_rs_sdk_test::{marine_test, CallParameters};
    use std::fs::{read_to_string, File};
    use std::io::Write;
    use std::path::Path;

    // Here we provide constant values for particle parameters.
    // They are required, since they're used to construct the correct path to the particle vault.
    const PARTICLE_ID: &str = "test_id";
    const TOKEN: &str = "token";

    // This is the path to the vault directory. Note that's a directory not for a single particle,
    // but for all particles.
    const VAULT_TEMP: &str = "./test_artifacts/temp";
    // On the other hand, this is a vault of the specific particle. Note that it's a subdirectory
    // of `VAULT_TEMP` and contains `PARTICLE_ID` and `TOKEN` in its file name.
    const PARTICLE_VAULT: &str = "./test_artifacts/temp/test_id-token";
    const VIRTUAL_VAULT: &str = "/tmp/vault/test_id-token";

    // Here, since we work this the filesystem in tests, we need to prepare directory
    // structure manually for testing. On deployment, all the directories will be created automatically.
    //
    // We need to clear manually after each run because it's impossible to set a temporary directory
    // as a module directory due to wasm limitations.
    fn before_each() {
        std::fs::create_dir_all(PARTICLE_VAULT).expect(&format!("create {PARTICLE_VAULT} failed"));
    }

    fn after_each() {
        std::fs::remove_dir_all(PARTICLE_VAULT).expect(&format!("remove {PARTICLE_VAULT} failed"));
    }
    fn after_all() {
        std::fs::remove_dir_all(VAULT_TEMP).expect(&format!("remove {VAULT_TEMP} failed"));
    }

    fn particle_cp() -> CallParameters {
        let mut cp = CallParameters::default();
        cp.particle.id = PARTICLE_ID.to_string();
        cp.particle.token = TOKEN.to_string();
        cp
    }

    fn enable_logger() {
        ::env_logger::builder()
            .filter_level(log::LevelFilter::Off)
            .filter_module("ls_effector", log::LevelFilter::Debug)
            .filter_module("wasmer_interface_types_fl", log::LevelFilter::Off)
            .filter_module("marine_core", log::LevelFilter::Off)
            .is_test(true)
            .try_init()
            .unwrap();
    }

    #[marine_test(config_path = "../test_artifacts/Config.toml")]
    fn test_curl_get_file_url(curl: marine_test_env::curl_effector::ModuleInterface) {
        enable_logger();
        let cp = particle_cp();

        let target_secrets = format!("{VAULT_TEMP}/secrets.json");
        let mut input_file = File::create(&target_secrets).unwrap();
        writeln!(input_file, "secret").unwrap();
        let full_target_secrets_path = Path::new(&target_secrets).canonicalize().unwrap();

        let input_request = marine_test_env::curl_effector::CurlRequest {
            url: format!("file://{}", full_target_secrets_path.display()),
            headers: vec![marine_test_env::curl_effector::HttpHeader {
                name: "content-type".to_string(),
                value: "application/json".to_string(),
            }],
        };
        let result = curl.curl_get_cp(input_request.clone(), "output.json".to_string(), cp.clone());
        assert!(!result.success, "forbidden url request must fail");

        let output_real_file = format!("./{PARTICLE_VAULT}/output.json");
        let output_real_file = Path::new(&output_real_file);
        assert!(
            !output_real_file.exists(),
            "output file must NOT be even created"
        );
    }

    #[marine_test(config_path = "../test_artifacts/Config.toml")]
    fn test_curl_post(curl: marine_test_env::curl_effector::ModuleInterface) {
        let opts = mockito::ServerOpts {
            port: 8080,
            ..Default::default()
        };
        let mut server = mockito::Server::new_with_opts(opts);
        let url = server.url();
        let expected_input = "{\"a\": \"c\"}";
        let expected_output = "{\"a\": \"b\"}";
        let mock = server
            .mock("POST", "/")
            .match_body(expected_input)
            .expect(2)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(expected_output)
            .create();

        let cp = particle_cp();

        let input_real_file = format!("./{PARTICLE_VAULT}/input.json");
        let output_real_file = format!("./{PARTICLE_VAULT}/output.json");

        let mut input_file = File::create(input_real_file).unwrap();
        writeln!(input_file, "{}", expected_input).unwrap();

        let input_request = marine_test_env::curl_effector::CurlRequest {
            url: url.clone(),
            headers: vec![marine_test_env::curl_effector::HttpHeader {
                name: "content-type".to_string(),
                value: "application/json".to_string(),
            }],
        };
        let result = curl.curl_post_cp(
            input_request.clone(),
            "input.json".to_string(),
            "output.json".to_string(),
            cp.clone(),
        );
        assert!(result.success, "error: {}", result.error);

        let actual_output = read_to_string(Path::new(&output_real_file)).unwrap();
        assert_eq!(actual_output, expected_output);

        // Also check full paths
        let input_real_file2 = format!("./{PARTICLE_VAULT}/input2.json");
        let output_real_file2 = format!("./{PARTICLE_VAULT}/output2.json");

        let mut input_file = File::create(input_real_file2).unwrap();
        writeln!(input_file, "{}", expected_input).unwrap();

        let result = curl.curl_post_cp(
            input_request,
            format!("{VIRTUAL_VAULT}/input2.json"),
            format!("{VIRTUAL_VAULT}/output2.json"),
            cp,
        );
        assert!(result.success, "error: {}", result.error);

        let actual_output = read_to_string(Path::new(&output_real_file2)).unwrap();
        assert_eq!(actual_output, expected_output);

        mock.assert();
    }

    #[marine_test(config_path = "../test_artifacts/Config.toml")]
    fn test_curl_get(curl: marine_test_env::curl_effector::ModuleInterface) {
        let opts = mockito::ServerOpts {
            port: 8080,
            ..Default::default()
        };
        let mut server = mockito::Server::new_with_opts(opts);

        let url = server.url();

        let extected_output = "{\"a\": \"b\"}";
        let mock = server
            .mock("GET", "/")
            .expect(2)
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(extected_output)
            .create();

        let cp = particle_cp();
        let input_request = marine_test_env::curl_effector::CurlRequest {
            url: url.clone(),
            headers: vec![marine_test_env::curl_effector::HttpHeader {
                name: "content-type".to_string(),
                value: "application/json".to_string(),
            }],
        };
        let result = curl.curl_get_cp(input_request.clone(), "output.json".to_string(), cp.clone());
        assert!(result.success, "error: {}", result.error);

        let output_real_path = format!("./{PARTICLE_VAULT}/output.json");
        let actual_output = read_to_string(Path::new(&output_real_path)).unwrap();
        assert_eq!(actual_output, extected_output);

        // Also check full paths
        let result = curl.curl_get_cp(input_request, format!("{VIRTUAL_VAULT}/output2.json"), cp);
        assert!(result.success, "error: {}", result.error);

        let output_real_path = format!("./{PARTICLE_VAULT}/output2.json");
        let actual_output = read_to_string(Path::new(&output_real_path)).unwrap();
        assert_eq!(actual_output, extected_output);

        mock.assert();
    }
}
