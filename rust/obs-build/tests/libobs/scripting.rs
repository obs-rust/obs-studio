//! Real scripting lifecycle through the unchanged native API and bindings.
use obs_build::{NativeBuild, SharedLibrary, scripting};
use std::path::Path;

#[expect(
    clippy::too_many_lines,
    reason = "one block per scripting contract checked"
)]
pub fn exercise(
    build: &NativeBuild,
    root: &Path,
    artifacts: &scripting::Artifacts,
    cores: &[&Path],
    frontend: bool,
) {
    let output = build.output.join("script-scenarios");
    std::fs::create_dir_all(&output).unwrap();
    let output = output.canonicalize().unwrap();
    let source = output.join("probe.c");
    std::fs::write(&source, PROBE).unwrap();
    let mut target = SharedLibrary {
        name: "scripting-probe".into(),
        sources: vec![source],
        include_dirs: vec![
            root.into(),
            root.join("libobs"),
            root.join("shared/obs-scripting"),
            build.output.join("config"),
        ],
        dependencies: vec![cores[0].into(), artifacts.library.clone()],
        compile_options: vec!["-std=gnu17".into()],
        ..SharedLibrary::default()
    };
    target.include_dirs.extend(
        pkg_config::Config::new()
            .cargo_metadata(false)
            .probe("simde")
            .unwrap()
            .include_paths,
    );
    let executable = build.executable(&target).unwrap();
    let bin = artifacts.runtime.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let runner = bin.join(executable.file_name().unwrap());
    std::fs::copy(&executable, &runner).unwrap();
    let lua = output.join("native_smoke.lua");
    let python = output.join("native_smoke.py");
    let lua_frontend = if frontend {
        "\nassert(obs.obs_frontend_recording_active() == false)\n"
    } else {
        "\nassert(obs.obs_frontend_recording_active == nil)\n"
    };
    let python_frontend = if frontend {
        "\nassert obs.obs_frontend_recording_active() is False\n"
    } else {
        "\nassert not hasattr(obs, 'obs_frontend_recording_active')\n"
    };
    std::fs::write(&lua, format!("{LUA}{lua_frontend}")).unwrap();
    std::fs::write(&python, format!("{PYTHON}{python_frontend}")).unwrap();
    for core in cores {
        let config = tempfile::tempdir().unwrap();
        let mut paths = vec![
            core.parent().unwrap().to_owned(),
            artifacts.library.parent().unwrap().to_owned(),
        ];
        paths.extend(std::env::split_paths(
            &std::env::var_os("LD_LIBRARY_PATH").unwrap_or_default(),
        ));
        let result = std::process::Command::new(&runner)
            .arg(if artifacts.lua_module.is_some() {
                lua.as_os_str()
            } else {
                std::ffi::OsStr::new("-")
            })
            .arg(if artifacts.python_module.is_some() {
                python.as_os_str()
            } else {
                std::ffi::OsStr::new("-")
            })
            .arg(config.path())
            .arg(artifacts.lua_module.as_deref().unwrap_or(Path::new("-")))
            .arg(artifacts.python_module.as_deref().unwrap_or(Path::new("-")))
            .arg(core)
            .current_dir(config.path())
            .env("LD_LIBRARY_PATH", std::env::join_paths(paths).unwrap())
            .env_remove("PYTHONHOME")
            .env_remove("PYTHONPATH")
            .env_remove("LUA_PATH")
            .env_remove("LUA_CPATH")
            .output()
            .unwrap();
        let label = format!(
            "{}-lua{}-python{}-frontend{}.log",
            core.parent()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy(),
            u8::from(artifacts.lua_module.is_some()),
            u8::from(artifacts.python_module.is_some()),
            u8::from(frontend)
        );
        std::fs::write(
            output.join(label),
            format!(
                "{}\n{}",
                String::from_utf8_lossy(&result.stdout),
                String::from_utf8_lossy(&result.stderr)
            ),
        )
        .unwrap();
        assert!(
            result.status.success(),
            "scripting {}: {}\n{}\n{}",
            core.display(),
            result.status,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        println!(
            "cc::Build scripting defaults/update/properties/save/reload/unload passed: {}",
            core.display()
        );
    }
}

const LUA: &str = r#"
obs = obslua
function script_description() return 'native-lua' end
function script_defaults(settings) obs.obs_data_set_default_int(settings, 'seed', 41) end
function script_update(settings)
    obs.obs_data_set_int(settings, 'updated', obs.obs_data_get_int(settings, 'seed') + 1)
end
function script_save(settings)
    obs.obs_data_set_int(settings, 'saved', obs.obs_data_get_int(settings, 'updated') + 1)
end
function script_properties()
    local props = obs.obs_properties_create()
    obs.obs_properties_add_int(props, 'gain', 'Gain', -100, 100, 1)
    return props
end
function script_unload() obs.script_log(obs.LOG_INFO, 'native-unload') end
"#;

const PYTHON: &str = r#"
import obspython as obs
def script_description():
    return 'native-python'
def script_defaults(settings):
    obs.obs_data_set_default_int(settings, 'seed', 41)
def script_update(settings):
    obs.obs_data_set_int(settings, 'updated', obs.obs_data_get_int(settings, 'seed') + 1)
def script_save(settings):
    obs.obs_data_set_int(settings, 'saved', obs.obs_data_get_int(settings, 'updated') + 1)
def script_properties():
    props = obs.obs_properties_create()
    obs.obs_properties_add_int(props, 'gain', 'Gain', -100, 100, 1)
    return props
def script_unload():
    obs.script_log(obs.LOG_INFO, 'native-unload')
"#;

const PROBE: &str = r#"
#include <obs.h>
#include <obs-scripting.h>
#include <stdio.h>
#include <string.h>
#include <util/base.h>
#define CHECK(value) do { if (!(value)) { fprintf(stderr, "scripting check line %d: %s\n", __LINE__, #value); return 1; } } while (0)
static unsigned errors, unloads;
static bool module_loaded(const char *path)
{
    FILE *maps = fopen("/proc/self/maps", "r");
    if (!maps) return false;
    char line[8192];
    bool found = false;
    while (fgets(line, sizeof(line), maps))
        if (strstr(line, path)) { found = true; break; }
    fclose(maps);
    return found;
}
static void log_script(void *unused, obs_script_t *script, int level, const char *message)
{
    (void)unused; (void)script;
    if (level <= LOG_ERROR) ++errors;
    if (strstr(message, "native-unload")) ++unloads;
    fprintf(stderr, "script: %s\n", message);
}
static int exercise(const char *path, enum obs_script_lang language, const char *description, const char *module)
{
    obs_data_t *settings = obs_data_create();
    obs_script_t *script = obs_script_create(path, settings);
    obs_data_release(settings);
    CHECK(script && obs_script_loaded(script));
    CHECK(module_loaded(module));
    printf("cc::Build loaded scripting binding: %s\n", module);
    CHECK(obs_script_get_lang(script) == language);
    CHECK(strcmp(obs_script_get_description(script), description) == 0);
    settings = obs_script_get_settings(script);
    CHECK(obs_data_get_int(settings, "seed") == 41);
    CHECK(obs_data_get_int(settings, "updated") == 42);
    obs_data_release(settings);
    obs_properties_t *properties = obs_script_get_properties(script);
    CHECK(properties && obs_properties_get(properties, "gain"));
    CHECK(obs_property_get_type(obs_properties_get(properties, "gain")) == OBS_PROPERTY_INT);
    obs_properties_destroy(properties);
    settings = obs_data_create();
    obs_data_set_int(settings, "seed", 100);
    obs_script_update(script, settings);
    obs_data_release(settings);
    settings = obs_script_save(script);
    CHECK(obs_data_get_int(settings, "updated") == 101 && obs_data_get_int(settings, "saved") == 102);
    obs_data_release(settings);
    CHECK(obs_script_reload(script) && obs_script_loaded(script));
    CHECK(strcmp(obs_script_get_description(script), description) == 0);
    unsigned before = unloads;
    obs_script_destroy(script);
    CHECK(unloads == before + 1);
    CHECK(errors == 0);
    return 0;
}
int main(int argc, char **argv)
{
    CHECK(argc == 7);
    CHECK(obs_startup("en-US", argv[3], NULL));
    CHECK(module_loaded(argv[6]));
    obs_scripting_set_log_callback(log_script, NULL);
    CHECK(obs_scripting_load());
    bool lua = strcmp(argv[1], "-") != 0, python = strcmp(argv[2], "-") != 0;
    const char **formats = obs_scripting_supported_formats();
    unsigned count = 0;
    bool has_lua = false, has_python = false;
    while (formats[count]) {
        has_lua |= strcmp(formats[count], "lua") == 0;
        has_python |= strcmp(formats[count], "py") == 0;
        ++count;
    }
    CHECK(count == (unsigned)lua + (unsigned)python);
    CHECK(has_lua == lua && has_python == python);
    CHECK(obs_scripting_python_loaded() == python);
    if (lua) CHECK(exercise(argv[1], OBS_SCRIPT_LANG_LUA, "native-lua", argv[4]) == 0);
    if (python) CHECK(exercise(argv[2], OBS_SCRIPT_LANG_PYTHON, "native-python", argv[5]) == 0);
    obs_scripting_unload();
    obs_shutdown();
    puts("cc::Build scripting lifecycle complete");
    return 0;
}
"#;
