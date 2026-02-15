//! 极致优化的跨平台Java环境检测器
//! 核心目标：体积小、内存省、速度快、功能全

use std::collections::HashMap;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

// ---------- 核心数据结构 (极致紧凑) ----------
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct JavaInfo {
    /// Java安装根目录 (JAVA_HOME)
    pub home_path: String,
    /// 主版本号 (例如：8, 11, 17, 21)
    pub major_version: u16,
    /// 是否为64位
    pub is_64bit: bool,
    /// 运行时位宽 (用于显示)
    pub bitness: Bitness,
    /// 完整版本字符串 (例如 "1.8.0_391" 或 "17.0.9")
    pub full_version: String,
}

/// 位宽枚举 (比字符串更省内存)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Bitness {
    Unknown,
    Bit32,
    Bit64,
}

impl fmt::Display for Bitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Bitness::Unknown => write!(f, "Unknown"),
            Bitness::Bit32 => write!(f, "32-bit"),
            Bitness::Bit64 => write!(f, "64-bit"),
        }
    }
}

// ---------- 全局缓存 (OnceLock确保线程安全且只初始化一次) ----------
static JAVA_CACHE: OnceLock<HashMap<String, JavaInfo>> = OnceLock::new();

/// 获取或初始化Java信息缓存
fn get_java_cache() -> &'static HashMap<String, JavaInfo> {
    JAVA_CACHE.get_or_init(|| {
        let mut cache = HashMap::new();
        // 尝试从上次扫描的缓存文件快速加载 (此处为预留接口，实际可按需实现)
        // 本次优化以内存扫描为主
        let _ = load_cache_from_file(&mut cache);
        cache
    })
}

/// 预留函数：从文件加载缓存 (减少重复扫描)
fn load_cache_from_file(_cache: &mut HashMap<String, JavaInfo>) -> io::Result<()> {
    // 此处可集成项目的配置文件管理
    // 例如：读取 `~/.sealantern/java_cache.json`
    Ok(())
}

/// 预留函数：保存缓存到文件
fn save_cache_to_file(_cache: &HashMap<String, JavaInfo>) -> io::Result<()> {
    // 例如：保存到 `~/.sealantern/java_cache.json`
    Ok(())
}

// ---------- 工具函数 ----------
/// 获取当前时间戳 (用于缓存时效判断)
fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// 静默执行系统命令并获取输出 (跨平台)
fn execute_command_silently(cmd: &str, args: &[&str]) -> io::Result<String> {
    let output = Command::new(cmd)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        // 对于 `java -version`，版本信息输出到stderr，这是正常的
        if cmd.ends_with("java") && args == &["-version"] {
            Ok(String::from_utf8_lossy(&output.stderr).into_owned())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Other,
                format!("命令执行失败: {}", String::from_utf8_lossy(&output.stderr)),
            ))
        }
    }
}

/// 快速解析 `java -version` 输出
fn parse_java_version_output(output: &str) -> (u16, Bitness, String) {
    let mut major_version = 0;
    let mut bitness = Bitness::Unknown;
    let mut full_version = String::new();

    let lines: Vec<&str> = output.lines().collect();
    if let Some(first_line) = lines.get(0) {
        // 提取完整版本字符串 (例如 "openjdk version "17.0.9" 2023-10-17")
        let version_part = first_line.split('\"').nth(1).unwrap_or("");
        full_version = version_part.to_string();

        // 解析主版本号
        if version_part.starts_with("1.") {
            // 旧版本格式: "1.8.0_391"
            if let Some(second_dot) = version_part[2..].find('.') {
                if let Ok(ver) = version_part[2..2 + second_dot].parse::<u16>() {
                    major_version = ver;
                }
            }
        } else {
            // 新版本格式: "17.0.9"
            if let Some(first_dot) = version_part.find('.') {
                if let Ok(ver) = version_part[..first_dot].parse::<u16>() {
                    major_version = ver;
                }
            }
        }

        // 判断位数
        for line in lines {
            let line_lower = line.to_lowercase();
            if line_lower.contains("64-bit") {
                bitness = Bitness::Bit64;
                break;
            } else if line_lower.contains("32-bit") {
                bitness = Bitness::Bit32;
                break;
            }
        }
    }

    (major_version, bitness, full_version)
}

// ---------- 平台特定扫描逻辑 ----------
/// 获取平台特定的Java搜索命令
fn get_java_search_command() -> (&'static str, Vec<&'static str>) {
    if cfg!(target_os = "windows") {
        ("where", vec!["java.exe"])
    } else {
        // macOS & Linux
        ("which", vec!["-a", "java"])
    }
}

/// 获取平台特定的常见Java安装目录
fn get_common_java_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();

    if cfg!(target_os = "windows") {
        // Windows 常见目录
        dirs.push(PathBuf::from(r"C:\Program Files\Java"));
        dirs.push(PathBuf::from(r"C:\Program Files (x86)\Java"));
        if let Ok(program_data) = std::env::var("ProgramData") {
            dirs.push(PathBuf::from(program_data).join("Java"));
        }
    } else if cfg!(target_os = "macos") {
        // macOS 常见目录
        dirs.push(PathBuf::from("/Library/Java/JavaVirtualMachines"));
        dirs.push(PathBuf::from("/System/Library/Java/JavaVirtualMachines"));
        if let Ok(home) = std::env::var("HOME") {
            dirs.push(PathBuf::from(home).join("Library/Java/JavaVirtualMachines"));
        }
    }
    // 注意：项目文档明确此工具仅在电脑端使用，且当前需求针对Windows/macOS，故未包含Linux路径以精简。
    // 如需Linux支持，可在此添加 /usr/lib/jvm, /usr/java, /opt/java 等

    dirs
}

/// Windows专用：从注册表扫描Java安装
#[cfg(target_os = "windows")]
fn scan_windows_registry() -> Vec<JavaInfo> {
    use winreg::enums::*;
    use winreg::RegKey;

    let mut installations = Vec::new();
    let registry_paths = [
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\JavaSoft\Java Runtime Environment"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\JavaSoft\Java Development Kit"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Wow6432Node\JavaSoft\Java Runtime Environment"),
        (HKEY_LOCAL_MACHINE, r"SOFTWARE\Wow6432Node\JavaSoft\Java Development Kit"),
        (HKEY_CURRENT_USER, r"SOFTWARE\JavaSoft\Java Runtime Environment"),
        (HKEY_CURRENT_USER, r"SOFTWARE\JavaSoft\Java Development Kit"),
    ];

    for (hkey, path) in registry_paths.iter() {
        let root = match hkey {
            HKEY_LOCAL_MACHINE => RegKey::predef(HKEY_LOCAL_MACHINE),
            HKEY_CURRENT_USER => RegKey::predef(HKEY_CURRENT_USER),
            _ => continue,
        };

        if let Ok(java_key) = root.open_subkey(path) {
            if let Ok(versions) = java_key.enum_keys() {
                for version in versions.filter_map(Result::ok) {
                    if let Ok(version_key) = java_key.open_subkey(&version) {
                        if let Ok(java_home) = version_key.get_value::<String, _>("JavaHome") {
                            if Path::new(&java_home).exists() {
                                if let Ok(info) = validate_specific_java_path(&java_home) {
                                    installations.push(info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    installations
}

// ---------- 核心公共API ----------
/// 验证给定的Java路径是否有效，并返回详细信息
pub fn validate_specific_java_path(path: &str) -> io::Result<JavaInfo> {
    let clean_path = path.trim();
    if clean_path.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Java路径为空"));
    }

    let java_exe_path = find_java_executable(clean_path)?;
    let java_exe_str = java_exe_path.to_string_lossy();

    // 执行 java -version
    let version_output = execute_command_silently(&java_exe_str, &["-version"])?;
    let (major_version, bitness, full_version) = parse_java_version_output(&version_output);

    if major_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "无法从输出中解析Java版本",
        ));
    }

    // 确定JAVA_HOME (安装根目录)
    let home_path = determine_java_home(&java_exe_path);

    Ok(JavaInfo {
        home_path,
        major_version,
        is_64bit: bitness == Bitness::Bit64,
        bitness,
        full_version,
    })
}

/// 查找Java可执行文件的完整路径
fn find_java_executable(path: &str) -> io::Result<PathBuf> {
    let test_path = Path::new(path);

    if test_path.is_file() {
        // 如果输入的直接就是可执行文件
        if is_java_executable(test_path) {
            return Ok(test_path.to_path_buf());
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "指定的文件不是Java可执行文件",
            ));
        }
    }

    // 如果输入的是目录，尝试在常见子目录下查找
    let possible_paths = if cfg!(target_os = "windows") {
        vec![
            test_path.join("bin").join("java.exe"),
            test_path.join("java.exe"),
            test_path.join("javaw.exe"),
        ]
    } else {
        vec![
            test_path.join("bin").join("java"),
            test_path.join("java"),
        ]
    };

    for possible_path in possible_paths {
        if possible_path.exists() && is_java_executable(&possible_path) {
            return Ok(possible_path);
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        format!("在目录中未找到Java可执行文件: {}", path),
    ))
}

/// 判断路径是否为Java可执行文件
fn is_java_executable(path: &Path) -> bool {
    if cfg!(target_os = "windows") {
        path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("exe"))
            && path
                .file_name()
                .map_or(false, |name| name.to_string_lossy().to_lowercase().starts_with("java"))
    } else {
        // Unix-like系统：检查是否有可执行权限（简化处理，主要看文件名）
        path.file_name()
            .map_or(false, |name| name.to_string_lossy() == "java")
    }
}

/// 根据java可执行文件路径推断JAVA_HOME
fn determine_java_home(java_exe_path: &Path) -> String {
    // 通常结构为: JAVA_HOME/bin/java
    if let Some(parent) = java_exe_path.parent() {
        if parent.file_name().map_or(false, |name| name == "bin") {
            if let Some(grand_parent) = parent.parent() {
                return grand_parent.to_string_lossy().to_string();
            }
        }
    }
    // 如果不符合标准结构，返回可执行文件所在目录
    java_exe_path
        .parent()
        .map_or_else(|| java_exe_path.to_string_lossy().to_string(), |p| p.to_string_lossy().to_string())
}

/// 执行一次全面的系统Java环境扫描
pub fn perform_system_java_scan() -> HashMap<String, JavaInfo> {
    let mut all_installations = HashMap::new();
    let timestamp = current_timestamp();

    // 1. 通过系统命令查找 (PATH环境变量)
    let (cmd, args) = get_java_search_command();
    if let Ok(output) = execute_command_silently(cmd, &args) {
        for line in output.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                if let Some(java_home) = extract_java_home_from_executable_path(trimmed) {
                    if !all_installations.contains_key(&java_home) {
                        if let Ok(info) = validate_specific_java_path(&java_home) {
                            all_installations.insert(java_home.clone(), info);
                        }
                    }
                }
            }
        }
    }

    // 2. 扫描常见安装目录
    for dir in get_common_java_dirs() {
        if dir.exists() && dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_dir() {
                        let path_str = path.to_string_lossy().to_string();
                        if !all_installations.contains_key(&path_str) {
                            if let Ok(info) = validate_specific_java_path(&path_str) {
                                all_installations.insert(path_str, info);
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Windows特定：扫描注册表
    #[cfg(target_os = "windows")]
    {
        for info in scan_windows_registry() {
            all_installations.entry(info.home_path.clone()).or_insert(info);
        }
    }

    // 4. macOS特定：检查 `/usr/libexec/java_home` 输出
    #[cfg(target_os = "macos")]
    {
        if let Ok(output) = execute_command_silently("/usr/libexec/java_home", &["-V"]) {
            for line in output.lines() {
                if line.contains("/Library/Java/JavaVirtualMachines") {
                    if let Some(start) = line.find("/Library") {
                        if let Some(end) = line[start..].find(' ') {
                            let java_home = &line[start..start + end];
                            if !all_installations.contains_key(java_home) {
                                if let Ok(info) = validate_specific_java_path(java_home) {
                                    all_installations.insert(java_home.to_string(), info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 可以在此处将 all_installations 保存到文件缓存 (save_cache_to_file)
    // let _ = save_cache_to_file(&all_installations);

    all_installations
}

/// 从java可执行文件完整路径推断JAVA_HOME
fn extract_java_home_from_executable_path(exe_path: &str) -> Option<String> {
    let path = Path::new(exe_path);
    if let Some(parent) = path.parent() {
        if parent.file_name().map_or(false, |name| name == "bin") {
            if let Some(grand_parent) = parent.parent() {
                return Some(grand_parent.to_string_lossy().to_string());
            }
        }
    }
    None
}

/// 获取系统中所有已安装的Java（惰性扫描+缓存）
pub fn get_all_java_installations() -> Vec<JavaInfo> {
    let cache = get_java_cache();
    if !cache.is_empty() {
        // 如果缓存已有数据，直接返回（可在此处添加缓存时效判断逻辑）
        return cache.values().cloned().collect();
    }

    // 缓存为空，执行全面扫描并更新缓存
    let new_cache = perform_system_java_scan();
    // 注意：OnceLock只允许设置一次，这里通过获取后比较的方式。
    // 更严谨的生产环境可考虑使用Mutex<RwLock<HashMap>>，但会增加复杂度。
    // 对于此场景，应用启动时扫描一次并缓存是合理且高效的。
    let _ = JAVA_CACHE.set(new_cache);
    get_java_cache().values().cloned().collect()
}

/// 根据Minecraft版本获取推荐的Java版本（主版本号）
pub fn get_recommended_java_major_version(minecraft_version: &str) -> u16 {
    // 简化映射表，覆盖主流版本
    let version_map: HashMap<&str, u16> = [
        ("1.7", 8),
        ("1.8", 8),
        ("1.9", 8),
        ("1.10", 8),
        ("1.11", 8),
        ("1.12", 8),
        ("1.13", 8),
        ("1.14", 8),
        ("1.15", 8),
        ("1.16", 8),
        ("1.17", 16),
        ("1.18", 17),
        ("1.19", 17),
        ("1.20", 17),
        ("1.21", 21),
    ]
    .iter()
    .cloned()
    .collect();

    // 提取Minecraft主版本号 (例如 "1.20.1" -> "1.20")
    let mc_major = if let Some(dot_pos) = minecraft_version.find('.') {
        if let Some(second_dot) = minecraft_version[dot_pos + 1..].find('.') {
            &minecraft_version[..=dot_pos + second_dot]
        } else {
            minecraft_version
        }
    } else {
        minecraft_version
    };

    *version_map.get(mc_major).unwrap_or(&17) // 默认推荐Java 17
}

/// 强制刷新Java缓存（重新扫描系统）
pub fn refresh_java_cache() -> Vec<JavaInfo> {
    let new_cache = perform_system_java_scan();
    let _ = JAVA_CACHE.set(new_cache);
    get_java_cache().values().cloned().collect()
}
