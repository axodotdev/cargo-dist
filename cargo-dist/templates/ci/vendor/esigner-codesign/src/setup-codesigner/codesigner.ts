import * as core from '@actions/core';
import * as exec from '@actions/exec';
import * as tc from '@actions/tool-cache';

import fs, { mkdirSync, writeFileSync, chmodSync, readFileSync, existsSync } from 'fs';
import path from 'path';
import {
    CODESIGNTOOL_UNIX_RUN_CMD,
    CODESIGNTOOL_UNIX_SETUP,
    CODESIGNTOOL_WINDOWS_RUN_CMD,
    CODESIGNTOOL_WINDOWS_SETUP,
    PRODUCTION_ENVIRONMENT_NAME,
    INPUT_ENVIRONMENT_NAME,
    INPUT_JVM_MAX_MEMORY,
    WINDOWS,
    INPUT_DIR_PATH,
    INPUT_USERNAME,
    INPUT_PASSWORD,
    INPUT_CREDENTIAL_ID,
    INPUT_PROGRAM_NAME,
    ACTION_SCAN_CODE,
    CODESIGNTOOL_BASEPATH,
    INPUT_SIGNING_METHOD,
    SIGNING_METHOD_V1,
    CODESIGNTOOL_WINDOWS_SIGNING_COMMAND,
    CODESIGNTOOL_UNIX_SIGNING_COMMAND
} from '../constants';
import { CODESIGNTOOL_PROPERTIES, CODESIGNTOOL_DEMO_PROPERTIES } from '../config';

import { extractZip, getInput, getPlatform, listFiles, setCommand, userShell } from '../util';

export class CodeSigner {
    constructor() {}

    public async setup(): Promise<string> {
        const workingPath = path.resolve(process.cwd());
        listFiles(workingPath);

        const platform = getPlatform();
        let link = platform == WINDOWS ? CODESIGNTOOL_WINDOWS_SETUP : CODESIGNTOOL_UNIX_SETUP;
        let cmd = platform == WINDOWS ? CODESIGNTOOL_WINDOWS_RUN_CMD : CODESIGNTOOL_UNIX_RUN_CMD;

        const codesigner = path.resolve(process.cwd(), 'codesign');
        if (!existsSync(codesigner)) {
            mkdirSync(codesigner);
            core.info(`Created CodeSignTool base path ${codesigner}`);
        }

        let archivePath = process.env['CODESIGNTOOL_PATH'] ?? path.join(codesigner, CODESIGNTOOL_BASEPATH);
        if (!existsSync(archivePath)) {
            core.info(`Downloading CodeSignTool from ${link}`);
            const downloadedFile = await tc.downloadTool(link);
            await extractZip(downloadedFile, path.join(codesigner, CODESIGNTOOL_BASEPATH));
            core.info(`Extract CodeSignTool from download path ${downloadedFile} to ${codesigner}`);

            const archiveName = fs.readdirSync(codesigner)[0];
            archivePath = path.join(codesigner, archiveName);
            core.exportVariable(`CODESIGNTOOL_PATH`, archivePath);
        }

        core.info(`Archive name: ${CODESIGNTOOL_BASEPATH}, ${archivePath}`);
        listFiles(archivePath);

        const environment = core.getInput(INPUT_ENVIRONMENT_NAME) ?? PRODUCTION_ENVIRONMENT_NAME;
        const jvmMaxMemory = core.getInput(INPUT_JVM_MAX_MEMORY) ?? '2048M';
        const sourceConfig = environment == PRODUCTION_ENVIRONMENT_NAME ? CODESIGNTOOL_PROPERTIES : CODESIGNTOOL_DEMO_PROPERTIES;
        const signingMethod = core.getInput(INPUT_SIGNING_METHOD) ?? SIGNING_METHOD_V1;
        const destConfig = path.join(archivePath, 'conf/code_sign_tool.properties');

        core.info(`Write CodeSignTool config file ${sourceConfig} to ${destConfig}`);
        writeFileSync(destConfig, sourceConfig, { encoding: 'utf-8', flag: 'w' });

        core.info(`Set CODE_SIGN_TOOL_PATH env variable: ${archivePath}`);
        core.exportVariable(`CODE_SIGN_TOOL_PATH`, archivePath);

        let execCmd;
        if (signingMethod == SIGNING_METHOD_V1) {
            execCmd = path.join(archivePath, cmd);
            const execData = readFileSync(execCmd, { encoding: 'utf-8', flag: 'r' });
            const result = execData.replace(/java -jar/g, `java -Xmx${jvmMaxMemory} -jar`).replace(/\$@/g, `"\$@"`);
            core.info(`Exec Cmd Content: ${result}`);
            writeFileSync(execCmd, result, { encoding: 'utf-8', flag: 'w' });
            chmodSync(execCmd, '0755');
        } else {
            execCmd = platform == WINDOWS ? CODESIGNTOOL_WINDOWS_SIGNING_COMMAND : CODESIGNTOOL_UNIX_SIGNING_COMMAND;
            execCmd = execCmd.replace(/\${{ CODE_SIGN_TOOL_PATH }}/g, archivePath).replace(/\${{ JVM_MAX_MEMORY }}/g, jvmMaxMemory);
        }

        const shellCmd = userShell(signingMethod);
        core.info(`Shell Cmd: ${shellCmd}`);
        core.info(`Exec Cmd : ${execCmd}`);
        execCmd = shellCmd + ' ' + execCmd;
        execCmd = execCmd.trimStart().trimEnd();
        return execCmd;
    }

    public async scanCode(execCommand: string, action: string): Promise<boolean> {
        let command = `${ACTION_SCAN_CODE}`;
        command = setCommand(INPUT_USERNAME, command, action);
        command = setCommand(INPUT_PASSWORD, command, action);
        command = setCommand(INPUT_CREDENTIAL_ID, command, action);
        command = setCommand(INPUT_PROGRAM_NAME, command, action);

        let input_path = path.normalize(getInput(INPUT_DIR_PATH));
        const files = fs.readdirSync(input_path);
        for (const file of files) {
            let fullPath = path.join(input_path, file);
            let scan_code = `${command} -input_file_path="${fullPath}"`;
            scan_code = `${execCommand} ${scan_code}`;
            core.info(`CodeSigner scan code command: ${scan_code}`);
            const result = await exec.getExecOutput(scan_code, [], { windowsVerbatimArguments: false });
            if (
                result.stdout.includes('Error') ||
                result.stdout.includes('Exception') ||
                result.stdout.includes('Missing required option') ||
                result.stdout.includes('Unmatched arguments from') ||
                result.stderr.includes('Error') ||
                result.stderr.includes('Exception') ||
                result.stderr.includes('Missing required option') ||
                result.stderr.includes('Unmatched arguments from') ||
                result.stderr.includes('Unmatched argument')
            ) {
                return false;
            }
        }

        return true;
    }
}
