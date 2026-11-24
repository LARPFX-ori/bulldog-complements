//! OpenFX `.ofx.bundle` packager, exported symbol verifier, cross-suite descriptor
//! validator, and SPDX dependency license auditor for **Bulldog Complements**.

#![forbid(unsafe_code)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use bulldog_core::EffectDescriptor;
use bulldog_ofx::validate_suite;

struct SuiteBundleSpec {
    crate_name: &'static str,
    bundle_name: &'static str,
    display_name: &'static str,
}

const SUITES: &[SuiteBundleSpec] = &[
    SuiteBundleSpec {
        crate_name: "continuus",
        bundle_name: "BulldogContinuus",
        display_name: "Bulldog Continuus",
    },
    SuiteBundleSpec {
        crate_name: "sapphirus",
        bundle_name: "BulldogSapphirus",
        display_name: "Bulldog Sapphirus",
    },
    SuiteBundleSpec {
        crate_name: "dulcia_nitentia",
        bundle_name: "BulldogDulciaNitentia",
        display_name: "Bulldog Dulcia Nitentia",
    },
    SuiteBundleSpec {
        crate_name: "caeruleus_nebulosus",
        bundle_name: "BulldogCaeruleusNebulosus",
        display_name: "Bulldog Caeruleus Nebulosus",
    },
    SuiteBundleSpec {
        crate_name: "transitions_3d",
        bundle_name: "BulldogTransitions3d",
        display_name: "Bulldog 3D Transitions",
    },
    SuiteBundleSpec {
        crate_name: "tracker_man",
        bundle_name: "BulldogTrackerMan",
        display_name: "Bulldog TrackerMan",
    },
    SuiteBundleSpec {
        crate_name: "glass_brokers",
        bundle_name: "BulldogGlassBrokers",
        display_name: "Bulldog Glass Brokers",
    },
    SuiteBundleSpec {
        crate_name: "universum",
        bundle_name: "BulldogUniversum",
        display_name: "Bulldog Universum",
    },
    SuiteBundleSpec {
        crate_name: "particulae_legales",
        bundle_name: "BulldogParticulaeLegales",
        display_name: "Bulldog Particulae Legales",
    },
];

const ALLOWED_SPDX: &[&str] = &[
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "OFL-1.1",
    "Ubuntu-font-1.0",
    "Bitstream-Vera",
    "CC0-1.0",
    "CC-BY-4.0",
    "CC-BY-SA-4.0",
    "LicenseRef-Public-Domain",
];

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let cmd = args.first().map(String::as_str).unwrap_or("help");

    let result = match cmd {
        "package" => run_package(args.get(1..).unwrap_or(&[])),
        "validate-catalog" => run_validate_catalog(),
        "check-licenses" => run_check_licenses(),
        "help" | "--help" | "-h" => {
            print_usage();
            Ok(())
        }
        other => Err(format!("unknown subcommand '{other}'. Run with --help.")),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

fn print_usage() {
    println!(
        "bulldog_packager — OpenFX bundle builder & compliance verifier\n\
         \n\
         USAGE:\n\
           bulldog_packager package [--profile debug|release] [--out-dir <path>]\n\
           bulldog_packager validate-catalog\n\
           bulldog_packager check-licenses"
    );
}

fn all_suite_descriptors() -> Vec<EffectDescriptor> {
    let mut all = Vec::new();
    all.extend(continuus::suite_descriptors());
    all.extend(sapphirus::suite_descriptors());
    all.extend(dulcia_nitentia::suite_descriptors());
    all.extend(caeruleus_nebulosus::suite_descriptors());
    all.extend(transitions_3d::suite_descriptors());
    all.extend(tracker_man::suite_descriptors());
    all.extend(glass_brokers::suite_descriptors());
    all.extend(universum::suite_descriptors());
    all.extend(particulae_legales::suite_descriptors());
    all
}

fn run_validate_catalog() -> Result<(), String> {
    let descriptors = all_suite_descriptors();
    match validate_suite(&descriptors) {
        Ok(()) => {
            println!(
                "Catalog valid: {} OpenFX plugins across {} suites.",
                descriptors.len(),
                SUITES.len()
            );
            for d in &descriptors {
                println!(
                    "  - {} ({}, {:?}, {} params)",
                    d.identifier,
                    d.label,
                    d.context,
                    d.params.len()
                );
            }
            Ok(())
        }
        Err(errs) => {
            let mut msg = String::from("catalog validation failed:\n");
            for e in errs {
                msg.push_str(&format!("  - {e}\n"));
            }
            Err(msg)
        }
    }
}

fn run_package(args: &[String]) -> Result<(), String> {
    let mut profile = "debug".to_owned();
    let mut out_dir = PathBuf::from("dist");
    let mut idx = 0usize;

    while let Some(arg) = args.get(idx) {
        match arg.as_str() {
            "--profile" => {
                idx += 1;
                let Some(val) = args.get(idx) else {
                    return Err("--profile requires 'debug' or 'release'".to_owned());
                };
                if val != "debug" && val != "release" {
                    return Err(format!(
                        "invalid profile '{val}' (expected debug or release)"
                    ));
                }
                profile = val.clone();
            }
            "--out-dir" => {
                idx += 1;
                let Some(val) = args.get(idx) else {
                    return Err("--out-dir requires a directory path".to_owned());
                };
                out_dir = PathBuf::from(val);
            }
            other => {
                return Err(format!("unrecognized option '{other}'"));
            }
        }
        idx += 1;
    }

    run_validate_catalog()?;

    let (arch_dir, lib_prefix, lib_ext) = platform_ofx_layout()?;
    let target_dir = Path::new("target").join(&profile);

    fs::create_dir_all(&out_dir).map_err(|e| {
        format!(
            "failed to create output directory {}: {e}",
            out_dir.display()
        )
    })?;

    for suite in SUITES {
        let artifact_name = format!("{lib_prefix}bulldog_{}_ofx.{lib_ext}", suite.crate_name);
        let src_bin = target_dir.join(&artifact_name);
        if !src_bin.is_file() {
            return Err(format!(
                "missing compiled suite library '{}' (run `cargo build --workspace{}` first)",
                src_bin.display(),
                if profile == "release" {
                    " --release"
                } else {
                    ""
                }
            ));
        }

        let bundle_root = out_dir.join(format!("{}.ofx.bundle", suite.bundle_name));
        let contents_dir = bundle_root.join("Contents");
        let bin_dir = contents_dir.join(arch_dir);
        fs::create_dir_all(&bin_dir).map_err(|e| {
            format!(
                "failed to create bundle directory {}: {e}",
                bin_dir.display()
            )
        })?;

        let dst_ofx = bin_dir.join(format!("{}.ofx", suite.bundle_name));
        fs::copy(&src_bin, &dst_ofx).map_err(|e| {
            format!(
                "failed to copy {} to {}: {e}",
                src_bin.display(),
                dst_ofx.display()
            )
        })?;

        let plist_path = contents_dir.join("Info.plist");
        let plist = render_info_plist(suite);
        fs::write(&plist_path, plist)
            .map_err(|e| format!("failed to write {}: {e}", plist_path.display()))?;

        verify_ofx_binary_exports(&dst_ofx)?;
        println!(
            "Packaged & verified: {} -> {}",
            suite.display_name,
            dst_ofx.display()
        );
    }

    Ok(())
}

fn platform_ofx_layout() -> Result<(&'static str, &'static str, &'static str), String> {
    match (env::consts::OS, env::consts::ARCH) {
        ("windows", "x86_64") => Ok(("Win64", "", "dll")),
        ("windows", "x86") => Ok(("Win32", "", "dll")),
        ("windows", "aarch64") => Ok(("Win64-arm64", "", "dll")),
        ("linux", "x86_64") => Ok(("Linux-x86-64", "lib", "so")),
        ("linux", "aarch64") => Ok(("Linux-arm-64", "lib", "so")),
        ("macos", _) => Ok(("MacOS", "lib", "dylib")),
        (os, arch) => Err(format!("unsupported OpenFX host architecture: {os}/{arch}")),
    }
}

fn render_info_plist(suite: &SuiteBundleSpec) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>English</string>
    <key>CFBundleExecutable</key>
    <string>{bundle}</string>
    <key>CFBundleIdentifier</key>
    <string>br.com.bulldog.complements.{crate_name}</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>{display}</string>
    <key>CFBundlePackageType</key>
    <string>BNDL</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
</dict>
</plist>
"#,
        bundle = suite.bundle_name,
        crate_name = suite.crate_name,
        display = suite.display_name,
        version = env!("CARGO_PKG_VERSION"),
    )
}

fn verify_ofx_binary_exports(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path)
        .map_err(|e| format!("failed to read packaged binary {}: {e}", path.display()))?;

    let exports = if bytes.get(0..2) == Some(b"MZ") {
        parse_pe_exports(&bytes)
            .ok_or_else(|| format!("invalid PE32/PE32+ export directory in {}", path.display()))?
    } else {
        // ELF / Mach-O symbol verification: check exact NUL-delimited symbol names in binary.
        let mut found = Vec::new();
        for sym in ["OfxGetNumberOfPlugins", "OfxGetPlugin"] {
            let mut needle = Vec::with_capacity(sym.len() + 2);
            needle.push(0);
            needle.extend_from_slice(sym.as_bytes());
            needle.push(0);
            let mut mach_needle = Vec::with_capacity(sym.len() + 3);
            mach_needle.push(0);
            mach_needle.push(b'_');
            mach_needle.extend_from_slice(sym.as_bytes());
            mach_needle.push(0);
            if contains_subslice(&bytes, &needle) || contains_subslice(&bytes, &mach_needle) {
                found.push(sym.to_owned());
            }
        }
        found
    };

    for required in ["OfxGetNumberOfPlugins", "OfxGetPlugin"] {
        if !exports.iter().any(|name| name == required) {
            return Err(format!(
                "packaged binary {} does not export required OpenFX symbol '{required}'",
                path.display()
            ));
        }
    }
    Ok(())
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn read_u16_le(bytes: &[u8], off: usize) -> Option<u16> {
    let b0 = *bytes.get(off)?;
    let b1 = *bytes.get(off.checked_add(1)?)?;
    Some(u16::from_le_bytes([b0, b1]))
}

fn read_u32_le(bytes: &[u8], off: usize) -> Option<u32> {
    let slice = bytes.get(off..off.checked_add(4)?)?;
    Some(u32::from_le_bytes([
        *slice.first()?,
        *slice.get(1)?,
        *slice.get(2)?,
        *slice.get(3)?,
    ]))
}

fn parse_pe_exports(bytes: &[u8]) -> Option<Vec<String>> {
    let pe_off = read_u32_le(bytes, 0x3c)? as usize;
    if bytes.get(pe_off..pe_off.checked_add(4)?) != Some(b"PE\0\0") {
        return None;
    }
    let coff_off = pe_off.checked_add(4)?;
    let num_sections = read_u16_le(bytes, coff_off.checked_add(2)?)? as usize;
    let opt_size = read_u16_le(bytes, coff_off.checked_add(16)?)? as usize;
    let opt_off = coff_off.checked_add(20)?;
    let magic = read_u16_le(bytes, opt_off)?;
    let export_dir_entry_off = match magic {
        0x10b => opt_off.checked_add(96)?,  // PE32
        0x20b => opt_off.checked_add(112)?, // PE32+
        _ => return None,
    };
    let export_rva = read_u32_le(bytes, export_dir_entry_off)?;
    if export_rva == 0 {
        return Some(Vec::new());
    }

    let sec_table_off = opt_off.checked_add(opt_size)?;
    let mut sections = Vec::with_capacity(num_sections);
    for i in 0..num_sections {
        let s_off = sec_table_off.checked_add(i.checked_mul(40)?)?;
        let v_size = read_u32_le(bytes, s_off.checked_add(8)?)?;
        let v_addr = read_u32_le(bytes, s_off.checked_add(12)?)?;
        let raw_size = read_u32_le(bytes, s_off.checked_add(16)?)?;
        let raw_ptr = read_u32_le(bytes, s_off.checked_add(20)?)?;
        sections.push((v_addr, v_size.max(raw_size), raw_ptr));
    }

    let rva_to_off = |rva: u32| -> Option<usize> {
        for &(v_addr, span, raw_ptr) in &sections {
            if rva >= v_addr && rva < v_addr.saturating_add(span) {
                let delta = rva.checked_sub(v_addr)?;
                return raw_ptr.checked_add(delta).map(|v| v as usize);
            }
        }
        None
    };

    let exp_off = rva_to_off(export_rva)?;
    let num_names = read_u32_le(bytes, exp_off.checked_add(24)?)? as usize;
    let names_rva = read_u32_le(bytes, exp_off.checked_add(32)?)?;
    let names_off = rva_to_off(names_rva)?;

    let mut exports = Vec::with_capacity(num_names.min(4096));
    for i in 0..num_names.min(4096) {
        let name_rva = read_u32_le(bytes, names_off.checked_add(i.checked_mul(4)?)?)?;
        let name_off = rva_to_off(name_rva)?;
        let tail = bytes.get(name_off..)?;
        let nul_pos = tail.iter().position(|&b| b == 0)?;
        if let Ok(s) = std::str::from_utf8(tail.get(..nul_pos)?) {
            exports.push(s.to_owned());
        }
    }
    Some(exports)
}

fn run_check_licenses() -> Result<(), String> {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1"])
        .output()
        .map_err(|e| format!("failed to invoke `cargo metadata`: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "`cargo metadata` failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let json = String::from_utf8(output.stdout)
        .map_err(|e| format!("invalid UTF-8 from `cargo metadata`: {e}"))?;

    let packages = extract_packages_licenses(&json)?;
    if packages.is_empty() {
        return Err("no packages found in cargo metadata".to_owned());
    }

    let mut violations = Vec::new();
    for (name, license_opt) in &packages {
        match license_opt {
            Some(expr) if is_spdx_expression_allowed(expr) => {}
            Some(expr) => violations.push(format!("{name}: disallowed license '{expr}'")),
            None => violations.push(format!("{name}: missing SPDX license field")),
        }
    }

    if violations.is_empty() {
        println!(
            "License check passed: all {} resolved packages comply with AGENTS.md SPDX allowlist.",
            packages.len()
        );
        Ok(())
    } else {
        Err(format!(
            "license policy violations:\n  - {}",
            violations.join("\n  - ")
        ))
    }
}

/// Evaluates an SPDX expression containing `OR`, `AND`, `/`, and parentheses against
/// [`ALLOWED_SPDX`]: `A OR B` passes if either branch is allowed; `A AND B` requires both.
fn is_spdx_expression_allowed(expr: &str) -> bool {
    let normalized = expr.replace('/', " OR ");
    let tokens = tokenize_spdx(&normalized);
    if tokens.is_empty() {
        return false;
    }
    let mut pos = 0usize;
    let res = parse_spdx_or(&tokens, &mut pos);
    res && pos == tokens.len()
}

fn tokenize_spdx(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in input.chars() {
        if ch == '(' || ch == ')' {
            if !cur.is_empty() {
                out.push(core::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        } else if ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(core::mem::take(&mut cur));
            }
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn parse_spdx_or(tokens: &[String], pos: &mut usize) -> bool {
    let mut lhs = parse_spdx_and(tokens, pos);
    while tokens.get(*pos).map(String::as_str) == Some("OR") {
        *pos += 1;
        let rhs = parse_spdx_and(tokens, pos);
        lhs = lhs || rhs;
    }
    lhs
}

fn parse_spdx_and(tokens: &[String], pos: &mut usize) -> bool {
    let mut lhs = parse_spdx_atom(tokens, pos);
    while tokens.get(*pos).map(String::as_str) == Some("AND") {
        *pos += 1;
        let rhs = parse_spdx_atom(tokens, pos);
        lhs = lhs && rhs;
    }
    lhs
}

fn parse_spdx_atom(tokens: &[String], pos: &mut usize) -> bool {
    let Some(tok) = tokens.get(*pos) else {
        return false;
    };
    if tok == "(" {
        *pos += 1;
        let inner = parse_spdx_or(tokens, pos);
        if tokens.get(*pos).map(String::as_str) == Some(")") {
            *pos += 1;
            inner
        } else {
            false
        }
    } else {
        *pos += 1;
        ALLOWED_SPDX.contains(&tok.as_str())
    }
}

/// Minimal dependency-free parser extracting `(name, license)` for each package object
/// in the top-level `"packages"` array of `cargo metadata --format-version 1`.
fn extract_packages_licenses(json: &str) -> Result<Vec<(String, Option<String>)>, String> {
    let bytes = json.as_bytes();
    let mut idx = find_key_value_start(bytes, b"\"packages\"")
        .ok_or_else(|| "cargo metadata output missing \"packages\" array".to_owned())?;
    skip_ws(bytes, &mut idx);
    if bytes.get(idx) != Some(&b'[') {
        return Err("expected '[' after \"packages\"".to_owned());
    }
    idx += 1;

    let mut results = Vec::new();
    loop {
        skip_ws(bytes, &mut idx);
        match bytes.get(idx) {
            Some(&b']') => break,
            Some(&b'{') => {
                let (name, license) = parse_package_object(bytes, &mut idx)?;
                results.push((name, license));
                skip_ws(bytes, &mut idx);
                if bytes.get(idx) == Some(&b',') {
                    idx += 1;
                }
            }
            _ => return Err("unexpected token inside \"packages\" array".to_owned()),
        }
    }
    Ok(results)
}

fn parse_package_object(bytes: &[u8], idx: &mut usize) -> Result<(String, Option<String>), String> {
    if bytes.get(*idx) != Some(&b'{') {
        return Err("expected '{' for package object".to_owned());
    }
    *idx += 1;
    let mut name: Option<String> = None;
    let mut license: Option<String> = None;

    loop {
        skip_ws(bytes, idx);
        if bytes.get(*idx) == Some(&b'}') {
            *idx += 1;
            break;
        }
        let key = parse_json_string(bytes, idx)?;
        skip_ws(bytes, idx);
        if bytes.get(*idx) != Some(&b':') {
            return Err("expected ':' after JSON object key".to_owned());
        }
        *idx += 1;
        skip_ws(bytes, idx);

        if key == "name" {
            name = Some(parse_json_string(bytes, idx)?);
        } else if key == "license" {
            if bytes.get(*idx) == Some(&b'"') {
                license = Some(parse_json_string(bytes, idx)?);
            } else {
                skip_json_value(bytes, idx)?;
            }
        } else {
            skip_json_value(bytes, idx)?;
        }

        skip_ws(bytes, idx);
        if bytes.get(*idx) == Some(&b',') {
            *idx += 1;
        }
    }

    Ok((name.unwrap_or_else(|| "<unknown>".to_owned()), license))
}

fn find_key_value_start(bytes: &[u8], key: &[u8]) -> Option<usize> {
    let mut idx = 0usize;
    let mut depth = 0usize;
    while idx < bytes.len() {
        match *bytes.get(idx)? {
            b'"' => {
                let start = idx;
                let _ = parse_json_string(bytes, &mut idx).ok()?;
                if depth == 1 && bytes.get(start..idx) == Some(key) {
                    skip_ws(bytes, &mut idx);
                    if bytes.get(idx) == Some(&b':') {
                        return Some(idx + 1);
                    }
                }
            }
            b'{' | b'[' => {
                depth += 1;
                idx += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                idx += 1;
            }
            _ => idx += 1,
        }
    }
    None
}

fn skip_ws(bytes: &[u8], idx: &mut usize) {
    while let Some(&b) = bytes.get(*idx) {
        if b.is_ascii_whitespace() {
            *idx += 1;
        } else {
            break;
        }
    }
}

fn parse_json_string(bytes: &[u8], idx: &mut usize) -> Result<String, String> {
    if bytes.get(*idx) != Some(&b'"') {
        return Err("expected '\"' starting JSON string".to_owned());
    }
    *idx += 1;
    let mut out = String::new();
    while let Some(&b) = bytes.get(*idx) {
        *idx += 1;
        match b {
            b'"' => return Ok(out),
            b'\\' => {
                let Some(&esc) = bytes.get(*idx) else {
                    return Err("unterminated escape sequence in JSON string".to_owned());
                };
                *idx += 1;
                match esc {
                    b'"' => out.push('"'),
                    b'\\' => out.push('\\'),
                    b'/' => out.push('/'),
                    b'n' => out.push('\n'),
                    b'r' => out.push('\r'),
                    b't' => out.push('\t'),
                    b'u' => {
                        // Skip 4 hex digits safely.
                        *idx = idx.saturating_add(4);
                        out.push(' ');
                    }
                    _ => out.push(esc as char),
                }
            }
            other => out.push(other as char),
        }
    }
    Err("unterminated JSON string".to_owned())
}

fn skip_json_value(bytes: &[u8], idx: &mut usize) -> Result<(), String> {
    skip_ws(bytes, idx);
    let Some(&b) = bytes.get(*idx) else {
        return Err("unexpected EOF in JSON value".to_owned());
    };
    match b {
        b'"' => {
            let _ = parse_json_string(bytes, idx)?;
            Ok(())
        }
        b'{' | b'[' => {
            let mut depth = 0usize;
            while let Some(&cur) = bytes.get(*idx) {
                match cur {
                    b'"' => {
                        let _ = parse_json_string(bytes, idx)?;
                    }
                    b'{' | b'[' => {
                        depth += 1;
                        *idx += 1;
                    }
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);
                        *idx += 1;
                        if depth == 0 {
                            return Ok(());
                        }
                    }
                    _ => *idx += 1,
                }
            }
            Err("unterminated nested JSON container".to_owned())
        }
        _ => {
            while let Some(&cur) = bytes.get(*idx) {
                if matches!(cur, b',' | b'}' | b']') || cur.is_ascii_whitespace() {
                    break;
                }
                *idx += 1;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spdx_expression_evaluator_respects_allowlist() {
        assert!(is_spdx_expression_allowed("MIT OR Apache-2.0"));
        assert!(is_spdx_expression_allowed("MIT/Apache-2.0"));
        assert!(is_spdx_expression_allowed(
            "(MIT OR Apache-2.0) AND BSD-3-Clause"
        ));
        assert!(is_spdx_expression_allowed("Unlicense OR MIT"));
        assert!(!is_spdx_expression_allowed("GPL-3.0-only"));
        assert!(!is_spdx_expression_allowed("MIT AND GPL-2.0-only"));
        assert!(!is_spdx_expression_allowed(""));
    }
}
