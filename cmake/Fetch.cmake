# Pinned tarballs, downloaded once into a shared folder and checked against
# their SHA-256, then handed to FetchContent as local files. Every build tree
# (the debug, asan, tsan and fuzz presets, and Cargo's) reuses the one copy,
# and a failed download is retried, so a brief outage at GitHub doesn't fail
# a build (CI run #9 lost two presets to 502s from github.com/…/archive).
#
# anomp_download (URL SHA256 OUT_VAR) also works in script mode (cmake -P),
# which is how scripts/tests/test_fetch_cmake.py tests it.

if(NOT DEFINED ANOMP_DOWNLOAD_DIR)
    set(ANOMP_DOWNLOAD_DIR "${CMAKE_CURRENT_LIST_DIR}/../build/_downloads")
endif()
cmake_path(NORMAL_PATH ANOMP_DOWNLOAD_DIR)
if(NOT DEFINED ANOMP_DOWNLOAD_ATTEMPTS)
    set(ANOMP_DOWNLOAD_ATTEMPTS 3)
endif()
# Seconds before the second attempt; doubled before each one after it.
if(NOT DEFINED ANOMP_DOWNLOAD_RETRY_DELAY)
    set(ANOMP_DOWNLOAD_RETRY_DELAY 5)
endif()

# Sets OUT_VAR to the local copy of URL, downloading it if no copy with
# that hash is there yet. Fails the configure if every attempt fails or
# the file's hash differs.
function(anomp_download url sha256 out_var)
    string(TOLOWER "${sha256}" sha256)
    cmake_path(GET url FILENAME name)
    set(file "${ANOMP_DOWNLOAD_DIR}/${sha256}-${name}")

    if(EXISTS "${file}")
        file(SHA256 "${file}" actual)
        if(actual STREQUAL sha256)
            set(${out_var} "${file}" PARENT_SCOPE)
            return()
        endif()
        message(STATUS "Replacing ${file}: its SHA-256 is ${actual}")
        file(REMOVE "${file}")
    endif()

    file(MAKE_DIRECTORY "${ANOMP_DOWNLOAD_DIR}")
    # A name of its own, so two configures at once never share a part file;
    # the rename into place is atomic.
    string(RANDOM LENGTH 8 suffix)
    set(part "${file}.${suffix}.part")
    set(delay ${ANOMP_DOWNLOAD_RETRY_DELAY})
    set(errors "")
    foreach(attempt RANGE 1 ${ANOMP_DOWNLOAD_ATTEMPTS})
        if(attempt GREATER 1)
            message(STATUS "Retrying ${url} in ${delay} s")
            execute_process(COMMAND "${CMAKE_COMMAND}" -E sleep ${delay})
            math(EXPR delay "${delay} * 2")
        endif()
        message(STATUS "Downloading ${url}")
        file(DOWNLOAD "${url}" "${part}" STATUS status TLS_VERIFY ON)
        list(GET status 0 code)
        if(code EQUAL 0)
            file(SHA256 "${part}" actual)
            if(actual STREQUAL sha256)
                file(RENAME "${part}" "${file}")
                set(${out_var} "${file}" PARENT_SCOPE)
                return()
            endif()
            # A wrong hash isn't an outage: retrying won't change the file.
            file(REMOVE "${part}")
            message(FATAL_ERROR "${url}: SHA-256 ${actual}, expected ${sha256}")
        endif()
        list(GET status 1 reason)
        list(APPEND errors "attempt ${attempt}: ${reason}")
        file(REMOVE "${part}")
    endforeach()
    list(JOIN errors "\n  " errors)
    message(FATAL_ERROR "Couldn't download ${url}:\n  ${errors}")
endfunction()

# FetchContent_Declare (NAME URL <url> URL_HASH SHA256=<hash> ...) with the
# tarball fetched through anomp_download; the remaining arguments go to
# FetchContent_Declare unchanged.
function(anomp_fetch_declare name)
    cmake_parse_arguments(PARSE_ARGV 1 arg "" "URL;URL_HASH" "")
    if(NOT arg_URL_HASH MATCHES "^SHA256=([0-9A-Fa-f]+)$")
        message(FATAL_ERROR "anomp_fetch_declare(${name}): URL_HASH must be SHA256=<hash>")
    endif()
    anomp_download("${arg_URL}" "${CMAKE_MATCH_1}" file)
    FetchContent_Declare(${name}
        URL "${file}"
        URL_HASH "${arg_URL_HASH}"
        DOWNLOAD_EXTRACT_TIMESTAMP OFF
        ${arg_UNPARSED_ARGUMENTS})
endfunction()
