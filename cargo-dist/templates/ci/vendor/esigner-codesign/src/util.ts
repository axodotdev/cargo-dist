import os, { userInfo } from 'os';
import path from 'path';
import * as fs from 'fs';
import * as semver from 'semver';
import * as core from '@actions/core';
import * as tc from '@actions/tool-cache';
import {
    INPUT_COMMAND,
    INPUT_CREDENTIAL_ID,
    INPUT_FILE_PATH,
    INPUT_DIR_PATH,
    INPUT_MALWARE_BLOCK,
    INPUT_OUTPUT_PATH,
    INPUT_OVERRIDE,
    INPUT_PASSWORD,
    INPUT_PROGRAM_NAME,
    INPUT_TOTP_SECRET,
    INPUT_USERNAME,
    MACOS,
    UNIX,
    WINDOWS,
    SUPPORT_COMMANDS,
    SIGNING_METHOD_V2
} from './constants';

export function getTempDir() {
    return process.env['RUNNER_TEMP'] || os.tmpdir();
}

export async function extractJdkFile(toolPath: string, extension?: string) {
    if (!extension) {
        extension = toolPath.endsWith('.tar.gz') ? 'tar.gz' : path.extname(toolPath);
        if (extension.startsWith('.')) {
            extension = extension.substring(1);
        }
    }

    switch (extension) {
        case 'tar.gz':
        case 'tar':
            return await tc.extractTar(toolPath);
        case 'zip':
            return await tc.extractZip(toolPath);
        default:
            return await tc.extract7z(toolPath);
    }
}

export async function extractZip(toolPath: string, destPath: string) {
    return await tc.extractZip(toolPath, destPath);
}

export function getDownloadArchiveExtension() {
    return process.platform === 'win32' ? 'zip' : 'tar.gz';
}

export function isVersionSatisfies(range: string, version: string): boolean {
    if (semver.valid(range)) {
        const semRange = semver.parse(range);
        if (semRange && semRange.build?.length > 0) {
            return semver.compareBuild(range, version) === 0;
        }
    }

    return semver.satisfies(version, range);
}

export function getToolCachePath(toolName: string, version: string, architecture: string) {
    const toolCacheRoot = process.env['RUNNER_TOOL_CACHE'] ?? '';
    const fullPath = path.join(toolCacheRoot, toolName, version, architecture);
    if (fs.existsSync(fullPath)) {
        return fullPath;
    }

    return null;
}

export function getPlatform(): string {
    switch (process.platform) {
        case 'darwin':
            return MACOS;
        case 'win32':
            return WINDOWS;
        default:
            return UNIX;
    }
}

export function listFiles(path: string): void {
    const files = fs.readdirSync(path);
    files.forEach(file => {
        core.debug(`File: ${file}`);
    });
}

export function inputCommands(action: string): string {
    let command = `${core.getInput(INPUT_COMMAND)}`;
    command = setCommand(INPUT_USERNAME, command, action);
    command = setCommand(INPUT_PASSWORD, command, action);
    command = setCommand(INPUT_CREDENTIAL_ID, command, action);
    command = setCommand(INPUT_TOTP_SECRET, command, action);
    command = setCommand(INPUT_PROGRAM_NAME, command, action);
    command = setCommand(INPUT_FILE_PATH, command, action);
    command = setCommand(INPUT_DIR_PATH, command, action);
    command = setCommand(INPUT_OUTPUT_PATH, command, action);
    command = setCommand(INPUT_OVERRIDE, command, action);
    command = setCommand(INPUT_MALWARE_BLOCK, command, action);
    return command;
}

export function getInput(inputKey: string) {
    return replaceEnv(core.getInput(inputKey));
}

export function setCommand(inputKey: string, command: string, action: string): string {
    let input = getInput(inputKey);
    if (input == '') {
        return command;
    }
    const supportCommands = SUPPORT_COMMANDS.get(action);
    if (!supportCommands?.includes(inputKey)) {
        return command;
    }

    if (inputKey == INPUT_USERNAME) {
        command = `${command} -username="${input}"`;
    } else if (inputKey == INPUT_PASSWORD) {
        command = `${command} -password="${input}"`;
    } else if (inputKey == INPUT_CREDENTIAL_ID) {
        command = `${command} -credential_id="${input}"`;
    } else if (inputKey == INPUT_TOTP_SECRET) {
        command = `${command} -totp_secret="${input}"`;
    } else if (inputKey == INPUT_PROGRAM_NAME) {
        command = `${command} -program_name="${input}"`;
    } else if (inputKey == INPUT_FILE_PATH) {
        input = path.normalize(input);
        command = `${command} -input_file_path="${input}"`;
    } else if (inputKey == INPUT_DIR_PATH) {
        input = path.normalize(input);
        command = `${command} -input_dir_path="${input}"`;
    } else if (inputKey == INPUT_OUTPUT_PATH) {
        input = path.normalize(input);
        if (fs.existsSync(input)) {
            core.info(`CodeSignTool output path ${input} already exist`);
        } else {
            core.info(`Creating CodeSignTool output path ${input}`);
            fs.mkdirSync(input);
        }
        command = `${command} -output_dir_path="${input}"`;
    } else if (inputKey == INPUT_MALWARE_BLOCK) {
        command = `${command} -malware_block=${input}`;
    } else if (inputKey == INPUT_OVERRIDE) {
        command = `${command} -override=${input}`;
    }
    return command;
}

export function replaceEnv(input: string): string {
    const variables = process.env;
    for (const envKey in variables) {
        // @ts-ignore
        input = input.replace('${' + envKey + '}', variables[envKey]);
    }
    return input;
}

export function userShell(signingMethod: string): string | null {
    const { env } = process;

    const platform = getPlatform();
    if (platform == WINDOWS) {
        return '';
    }
    if (signingMethod == SIGNING_METHOD_V2) {
        return '';
    }

    try {
        const shell = userInfo();
        if (shell) return shell.shell;
    } catch {
        //Ignored
    }

    if (platform === MACOS) {
        return env.SHELL ?? '/bin/zsh';
    }

    return env.SHELL ?? '/bin/sh';
}
