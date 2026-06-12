#ifndef UNICODE
#define UNICODE
#endif
#ifndef _UNICODE
#define _UNICODE
#endif

#include <stdbool.h>
#include <stdio.h>
#include <windows.h>
#include <shellapi.h>
#include <wchar.h>

#define ROOT_ENV_NAME L"NTE_GACHA_ROOT"
#define LAUNCHER_ENV_NAME L"NTE_GACHA_LAUNCHER"
#define CORE_RELATIVE_PATH L"\\bin\\nte-gacha-core.exe"

typedef struct {
    wchar_t *data;
    size_t length;
    size_t capacity;
} TextBuffer;

static bool bufferInit(TextBuffer *buffer, size_t capacity) {
    buffer->data = (wchar_t *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, capacity * sizeof(wchar_t));
    if (!buffer->data) {
        return false;
    }
    buffer->length = 0;
    buffer->capacity = capacity;
    return true;
}

static void bufferFree(TextBuffer *buffer) {
    if (buffer->data) {
        HeapFree(GetProcessHeap(), 0, buffer->data);
    }
    buffer->data = NULL;
    buffer->length = 0;
    buffer->capacity = 0;
}

static bool bufferEnsure(TextBuffer *buffer, size_t extra) {
    size_t required = buffer->length + extra + 1;
    if (required <= buffer->capacity) {
        return true;
    }

    size_t next = buffer->capacity;
    while (next < required) {
        next *= 2;
    }

    wchar_t *data = (wchar_t *)HeapReAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, buffer->data, next * sizeof(wchar_t));
    if (!data) {
        return false;
    }
    buffer->data = data;
    buffer->capacity = next;
    return true;
}

static bool bufferAppendChar(TextBuffer *buffer, wchar_t value) {
    if (!bufferEnsure(buffer, 1)) {
        return false;
    }
    buffer->data[buffer->length++] = value;
    buffer->data[buffer->length] = L'\0';
    return true;
}

static bool bufferAppendRepeat(TextBuffer *buffer, wchar_t value, size_t count) {
    if (!bufferEnsure(buffer, count)) {
        return false;
    }
    for (size_t index = 0; index < count; index++) {
        buffer->data[buffer->length++] = value;
    }
    buffer->data[buffer->length] = L'\0';
    return true;
}

static bool bufferAppendQuoted(TextBuffer *buffer, const wchar_t *argument) {
    if (!bufferAppendChar(buffer, L'"')) {
        return false;
    }

    size_t backslashes = 0;
    for (const wchar_t *cursor = argument; *cursor; cursor++) {
        if (*cursor == L'\\') {
            backslashes++;
            continue;
        }

        if (*cursor == L'"') {
            if (!bufferAppendRepeat(buffer, L'\\', backslashes * 2 + 1) || !bufferAppendChar(buffer, L'"')) {
                return false;
            }
            backslashes = 0;
            continue;
        }

        if (backslashes && !bufferAppendRepeat(buffer, L'\\', backslashes)) {
            return false;
        }
        backslashes = 0;

        if (!bufferAppendChar(buffer, *cursor)) {
            return false;
        }
    }

    if (backslashes && !bufferAppendRepeat(buffer, L'\\', backslashes * 2)) {
        return false;
    }
    return bufferAppendChar(buffer, L'"');
}

static wchar_t *currentExecutablePath(void) {
    DWORD capacity = MAX_PATH;
    for (;;) {
        wchar_t *path = (wchar_t *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, capacity * sizeof(wchar_t));
        if (!path) {
            return NULL;
        }

        DWORD length = GetModuleFileNameW(NULL, path, capacity);
        if (length == 0) {
            HeapFree(GetProcessHeap(), 0, path);
            return NULL;
        }
        if (length < capacity - 1) {
            return path;
        }

        HeapFree(GetProcessHeap(), 0, path);
        capacity *= 2;
    }
}

static wchar_t *parentDirectory(const wchar_t *path) {
    size_t length = wcslen(path);
    wchar_t *directory = (wchar_t *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, (length + 1) * sizeof(wchar_t));
    if (!directory) {
        return NULL;
    }
    wcscpy(directory, path);

    for (wchar_t *cursor = directory + length; cursor > directory; cursor--) {
        if (cursor[-1] == L'\\' || cursor[-1] == L'/') {
            cursor[-1] = L'\0';
            return directory;
        }
    }

    wcscpy(directory, L".");
    return directory;
}

static wchar_t *joinCorePath(const wchar_t *root) {
    size_t rootLength = wcslen(root);
    size_t suffixLength = wcslen(CORE_RELATIVE_PATH);
    wchar_t *path = (wchar_t *)HeapAlloc(GetProcessHeap(), HEAP_ZERO_MEMORY, (rootLength + suffixLength + 1) * sizeof(wchar_t));
    if (!path) {
        return NULL;
    }
    wcscpy(path, root);
    wcscat(path, CORE_RELATIVE_PATH);
    return path;
}

static wchar_t *buildCommandLine(const wchar_t *selfPath) {
    int argc = 0;
    wchar_t **argv = CommandLineToArgvW(GetCommandLineW(), &argc);
    if (!argv) {
        return NULL;
    }

    TextBuffer buffer;
    if (!bufferInit(&buffer, 1024)) {
        LocalFree(argv);
        return NULL;
    }

    bool ok = bufferAppendQuoted(&buffer, selfPath);
    for (int index = 1; ok && index < argc; index++) {
        ok = bufferAppendChar(&buffer, L' ') && bufferAppendQuoted(&buffer, argv[index]);
    }

    LocalFree(argv);
    if (!ok) {
        bufferFree(&buffer);
        return NULL;
    }
    return buffer.data;
}

int wmain(void) {
    int exitCode = 127;
    wchar_t *selfPath = currentExecutablePath();
    wchar_t *root = selfPath ? parentDirectory(selfPath) : NULL;
    wchar_t *corePath = root ? joinCorePath(root) : NULL;
    wchar_t *commandLine = selfPath ? buildCommandLine(selfPath) : NULL;

    if (!selfPath || !root || !corePath || !commandLine) {
        fwprintf(stderr, L"failed to initialize nte-gacha wrapper\n");
        goto cleanup;
    }

    if (!SetEnvironmentVariableW(ROOT_ENV_NAME, root)) {
        fwprintf(stderr, L"failed to set %ls\n", ROOT_ENV_NAME);
        goto cleanup;
    }
    if (!SetEnvironmentVariableW(LAUNCHER_ENV_NAME, selfPath)) {
        fwprintf(stderr, L"failed to set %ls\n", LAUNCHER_ENV_NAME);
        goto cleanup;
    }

    STARTUPINFOW startupInfo;
    PROCESS_INFORMATION processInfo;
    ZeroMemory(&startupInfo, sizeof(startupInfo));
    ZeroMemory(&processInfo, sizeof(processInfo));
    startupInfo.cb = sizeof(startupInfo);

    BOOL started = CreateProcessW(
        corePath,
        commandLine,
        NULL,
        NULL,
        TRUE,
        0,
        NULL,
        root,
        &startupInfo,
        &processInfo);

    if (!started) {
        fwprintf(stderr, L"failed to start %ls: error=%lu\n", corePath, GetLastError());
        exitCode = 1;
        goto cleanup;
    }

    WaitForSingleObject(processInfo.hProcess, INFINITE);

    DWORD childExitCode = 1;
    if (GetExitCodeProcess(processInfo.hProcess, &childExitCode)) {
        exitCode = (int)childExitCode;
    } else {
        fwprintf(stderr, L"failed to read child exit code: error=%lu\n", GetLastError());
        exitCode = 1;
    }

    CloseHandle(processInfo.hThread);
    CloseHandle(processInfo.hProcess);

cleanup:
    if (commandLine) {
        HeapFree(GetProcessHeap(), 0, commandLine);
    }
    if (corePath) {
        HeapFree(GetProcessHeap(), 0, corePath);
    }
    if (root) {
        HeapFree(GetProcessHeap(), 0, root);
    }
    if (selfPath) {
        HeapFree(GetProcessHeap(), 0, selfPath);
    }
    return exitCode;
}
