# obs_cross_release_download(<what> <owner/repo> <tag> <asset> <file> <sha256>)
#
# Downloads a GitHub release asset to <file> and checks its SHA-256. With a
# token in GITHUB_TOKEN (or GH_TOKEN), as a CI job has, it asks the REST API
# for the asset as an authenticated client: GitHub can answer anonymous
# release downloads with 504 for hours while authenticated requests work
# (2026-11-09, #4209). Without a token (a local run) it uses the anonymous
# download URL. The token goes only in a header, never in a URL or a message;
# curl does not forward it when the API redirects to the signed asset host.
#
# OBS_CROSS_GITHUB_WEB and OBS_CROSS_GITHUB_API override the hosts for
# ci/test_cross_release_download.py.

include_guard(GLOBAL)

if(NOT DEFINED OBS_CROSS_GITHUB_WEB)
  set(OBS_CROSS_GITHUB_WEB "https://github.com")
endif()
if(NOT DEFINED OBS_CROSS_GITHUB_API)
  set(OBS_CROSS_GITHUB_API "https://api.github.com")
endif()

function(
  obs_cross_release_download
  what
  repo
  tag
  asset
  file
  sha256
)
  set(token "$ENV{GITHUB_TOKEN}")
  if(token STREQUAL "")
    set(token "$ENV{GH_TOKEN}")
  endif()
  if(token STREQUAL "")
    file(
      DOWNLOAD "${OBS_CROSS_GITHUB_WEB}/${repo}/releases/download/${tag}/${asset}"
      "${file}"
      EXPECTED_HASH SHA256=${sha256}
      STATUS status
    )
  else()
    set(release "${file}.release.json")
    file(
      DOWNLOAD "${OBS_CROSS_GITHUB_API}/repos/${repo}/releases/tags/${tag}"
      "${release}"
      HTTPHEADER "Authorization: Bearer ${token}"
      HTTPHEADER "Accept: application/vnd.github+json"
      HTTPHEADER "X-GitHub-Api-Version: 2022-11-28"
      STATUS status
    )
    list(GET status 0 code)
    set(url "")
    if(code EQUAL 0)
      file(READ "${release}" json)
      string(JSON count ERROR_VARIABLE json_error LENGTH "${json}" assets)
      if(json_error)
        set(count 0)
      endif()
      if(count GREATER 0)
        math(EXPR last "${count} - 1")
        foreach(i RANGE ${last})
          string(JSON name GET "${json}" assets ${i} name)
          if(name STREQUAL asset)
            string(JSON url GET "${json}" assets ${i} url)
            break()
          endif()
        endforeach()
      endif()
      if(url STREQUAL "")
        set(status "1;release ${tag} of ${repo} has no asset ${asset}")
      endif()
    endif()
    file(REMOVE "${release}")
    if(NOT url STREQUAL "")
      file(
        DOWNLOAD "${url}"
        "${file}"
        HTTPHEADER "Authorization: Bearer ${token}"
        HTTPHEADER "Accept: application/octet-stream"
        HTTPHEADER "X-GitHub-Api-Version: 2022-11-28"
        EXPECTED_HASH SHA256=${sha256}
        STATUS status
      )
    endif()
  endif()
  list(GET status 0 code)
  if(NOT code EQUAL 0)
    file(REMOVE "${file}")
    message(FATAL_ERROR "Downloading the ${what} failed: ${status}")
  endif()
endfunction()
