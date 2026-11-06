#!/usr/bin/env -S uv run --script
"""DeckLink SDK type selection and unchanged string adapters (#862)."""
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import unittest

import platform_ifdef_ratchet as ratchet
from test_settings_platform_facade import function

ROOT = Path(__file__).resolve().parent.parent
PLUGIN = ROOT / 'plugins/decklink'


def include_command():
    text = (PLUGIN / 'CMakeLists.txt').read_text()
    match = re.search(r'target_include_directories\(\s*decklink-sdk\b', text)
    if not match:
        return ''
    start = match.start(); end = text.index('(', start) + 1; depth = 1
    while depth:
        depth += (text[end] == '(') - (text[end] == ')'); end += 1
    return text[start:end]


class DecklinkPlatformHeadersTest(unittest.TestCase):
    def test_shared_header_has_no_host_selection(self):
        self.assertEqual(ratchet.count_platform_conditionals((PLUGIN / 'platform.hpp').read_text()), [])

    def test_build_selects_host_headers(self):
        self.assertTrue(include_command(), 'SDK interface has no host include selection')
        for host in ('windows', 'macos', 'linux'):
            self.assertTrue((PLUGIN / f'hosts/{host}/decklink-platform.hpp').is_file())

    def test_fixture_reads_system_sdk_includes(self):
        self.assertIn('SYSTEM', include_command())
        for directory in ('win', 'mac', 'linux'):
            self.assertIn(f'/{directory}/decklink-sdk', include_command())

    def test_header_types_and_conversion_behavior(self):  # noqa: C901
        compiler = shutil.which('c++')
        if not compiler:
            self.skipTest('C++ compiler unavailable; native plugin builds remain required')
        for system, host, flags in (
            ('Windows', 'windows', ['-U__linux__', '-D_WIN32']),
            ('Darwin', 'macos', ['-U__linux__', '-D__APPLE__']),
            ('Linux', 'linux', ['-D__linux__']),
            ('FreeBSD', 'linux', ['-U__linux__']),
        ):
            with self.subTest(system=system), tempfile.TemporaryDirectory(prefix='obs-decklink-') as tmp:
                base = Path(tmp)
                def write(path, text):
                    p=base/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_text(text)
                write('plugins/decklink/platform.hpp', (PLUGIN / 'platform.hpp').read_text())
                for target in PLUGIN.glob('hosts/*/decklink-platform.hpp'):
                    write(str(target.relative_to(PLUGIN)), target.read_text())
                sdk = '#pragma once\nstruct IDeckLinkDiscovery;struct IDeckLinkIterator;struct IDeckLinkVideoConversion;\n'
                write('DeckLinkAPI.h', sdk)
                for directory in ('win', 'mac', 'linux'):
                    write(f'plugins/decklink/{directory}/decklink-sdk/DeckLinkAPI.h', sdk)
                    write(f'plugins/decklink/{directory}/decklink-sdk/DeckLinkAPIVersion.h', '#pragma once\n')
                write('combaseapi.h', '#pragma once\nusing BOOL=int;using BSTR=wchar_t*;using REFIID=const int&;inline const int IID_IUnknown=1;\n')
                write('CoreFoundation/CoreFoundation.h', '#pragma once\nstruct CFString;using CFStringRef=const CFString*;\n')
                for name in ('HRError', 'ComPtr'):
                    write(f'util/windows/{name}.hpp', '#pragma once\n')
                selected = base
                command = include_command()
                if command:
                    write('CMakeLists.txt', 'cmake_minimum_required(VERSION 3.28)\nproject(decklink_header NONE)\n'
                          'add_library(decklink-sdk INTERFACE)\n' + command + '\n'
                          'file(GENERATE OUTPUT "${CMAKE_BINARY_DIR}/selection.txt" CONTENT '
                          '"$<TARGET_GENEX_EVAL:decklink-sdk,$<TARGET_PROPERTY:decklink-sdk,INTERFACE_INCLUDE_DIRECTORIES>>")\n')
                    result = subprocess.run(['cmake', '-S', tmp, '-B', str(base/'build'),
                                             '-DCMAKE_SYSTEM_NAME='+system], capture_output=True,text=True)
                    self.assertEqual(result.returncode,0,result.stderr)
                    directories=[p for p in (base/'build/selection.txt').read_text().split(';') if p]
                    self.assertEqual(directories,[str(base/f'hosts/{host}')])
                    selected=Path(directories[0])
                preamble = '#include <string>\n#include <type_traits>\n#include <cassert>\n#include <cstring>\n#include <cstdlib>\n#include "plugins/decklink/platform.hpp"\n'
                if system=='FreeBSD':
                    write('types.cpp',preamble)
                    # BSD was listed in SDK CMake, but the original header never
                    # defined its string type. Preserve that unsupported case.
                    result=subprocess.run([compiler,'-std=c++17','-I',tmp,'-I',str(selected),*flags,
                                           '-fsyntax-only',str(base/'types.cpp')],capture_output=True,text=True)
                    self.assertNotEqual(result.returncode,0)
                    self.assertIn('decklink_string_t',result.stderr)
                    continue
                native={'windows':'win','macos':'mac','linux':'linux'}[host]
                body=function((PLUGIN/f'{native}/platform.cpp.hpp').read_text(), 'bool DeckLinkStringToStdString(')
                if host=='windows':
                    preamble+='''
static_assert(std::is_same_v<decklink_bool_t,BOOL>);static_assert(std::is_same_v<decklink_string_t,BSTR>);
static_assert(std::is_same_v<CFUUIDBytes,REFIID>);
static_assert(std::is_same_v<decltype(&CreateDeckLinkDiscoveryInstance),IDeckLinkDiscovery*(*)()>);
static_assert(std::is_same_v<decltype(&CreateDeckLinkIteratorInstance),IDeckLinkIterator*(*)()>);
static_assert(std::is_same_v<decltype(&CreateVideoConversionInstance),IDeckLinkVideoConversion*(*)()>);
static int conversions=0;
namespace _com_util {char *ConvertBSTRToString(BSTR){++conversions;char *p=new char[5];std::strcpy(p,"text");return p;}}
'''
                    checks='std::string output="old";assert(!DeckLinkStringToStdString(nullptr,output));assert(output=="old"&&conversions==0);wchar_t input[]=L"text";assert(DeckLinkStringToStdString(input,output));assert(output=="text"&&conversions==1);'
                elif host=='macos':
                    preamble+='''
static_assert(std::is_same_v<decklink_bool_t,bool>);static_assert(std::is_same_v<decklink_string_t,CFStringRef>);
struct CFString{};static bool copyFails=false;static int freed=0,released=0;constexpr int kCFStringEncodingASCII=1;
char *cfstr_copy_cstr(CFStringRef,int encoding){assert(encoding==1);if(copyFails)return nullptr;char *p=new char[5];std::strcpy(p,"text");return p;}
void bfree(char *p){++freed;delete[]p;}void CFRelease(CFStringRef p){assert(p);++released;}
'''
                    checks='CFString input;std::string output="old";assert(DeckLinkStringToStdString(&input,output));assert(output=="text"&&freed==1&&released==1);copyFails=true;output="old";assert(!DeckLinkStringToStdString(&input,output));assert(output=="old"&&freed==2&&released==2);'
                else:
                    preamble+='''
static_assert(std::is_same_v<decklink_bool_t,bool>);static_assert(std::is_same_v<decklink_string_t,const char*>);
static int freed=0;void track_free(void *p){++freed;std::free(p);}
#define free track_free
'''
                    checks='std::string output="old";assert(!DeckLinkStringToStdString(nullptr,output));assert(output=="old"&&freed==0);for(const char *value:{"text","","\\xc3\\xa9"}){char *input=strdup(value);assert(DeckLinkStringToStdString(input,output));assert(output==value);}assert(freed==3);'
                write('types.cpp',preamble+body+'\nint main(){'+checks+'}\n')
                result=subprocess.run([compiler,'-std=c++17','-I',tmp,'-I',str(selected),*flags,
                                       str(base/'types.cpp'),'-o',str(base/'types')],capture_output=True,text=True)
                self.assertEqual(result.returncode,0,result.stderr)
                subprocess.run([str(base/'types')],check=True)


if __name__=='__main__':
    unittest.main()
