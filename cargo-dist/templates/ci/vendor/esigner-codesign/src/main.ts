import * as core from '@actions/core';
import * as exec from '@actions/exec';

import fs from 'fs';
import path from 'path';
import { ACTION_BATCH_SIGN, INPUT_CLEAN_LOGS, INPUT_COMMAND, INPUT_MALWARE_BLOCK, INPUT_SIGNING_METHOD, SIGNING_METHOD_V1, SIGNING_METHOD_V2, WINDOWS } from './constants';

import { CodeSigner } from './setup-codesigner/codesigner';
import { JavaDistribution } from './setup-jdk/installer';
import { getPlatform, inputCommands } from './util';

async function run(): Promise<void> {
    try {
        core.debug('Run CodeSigner');
        core.debug('Running ESigner.com CodeSign Action ====>');

        let action = `${core.getInput(INPUT_COMMAND)}`;
        let command = inputCommands(action);
        core.info(`Input Commands: ${command}`);

        let javaVersion = parseInt(process.env['JAVA_VERSION'] ?? '0');
        let javaHome = process.env['JAVA_HOME'] ?? '';
        core.info(`JDK home: ${javaHome}`);
        core.info(`JDK version: ${javaVersion}`);
        if (javaVersion < 11) {
            const distribution = new JavaDistribution();
            await distribution.setup();
            javaHome = process.env['JAVA_HOME'] ?? '';
        } else {
            core.info(`JDK is already installed ${javaHome}`);
        }

        const codesigner = new CodeSigner();
        let execCommand = await codesigner.setup();
        execCommand = execCommand.replace(/\${{ JAVA_HOME }}/g, `${javaHome}/bin/java`);
        command = `${execCommand} ${command}`;
        core.info(`CodeSigner Command: ${command}`);

        let malware_scan = `${core.getInput(INPUT_MALWARE_BLOCK, { required: false })}`;
        core.info(`Malware scan is: ${malware_scan.toUpperCase() == 'TRUE' ? 'enabled' : 'disabled'}`);
        if (action == ACTION_BATCH_SIGN && malware_scan.toUpperCase() == 'TRUE') {
            const scan_result = await codesigner.scanCode(execCommand, action);
            if (!scan_result) {
                core.info('');
                core.setFailed('Something Went Wrong. Please try again.');
                return;
            }
        }

        const result = await exec.getExecOutput(command, [], { windowsVerbatimArguments: false });

        const clean_logs = core.getBooleanInput(INPUT_CLEAN_LOGS);
        if (clean_logs) {
            const workingDir = path.dirname(command);
            const logsDir = path.join(workingDir, 'logs');
            fs.rmSync(logsDir, { recursive: true, force: true });
            core.info(`CodeSigner logs folder is deleted: ${logsDir}`);
        }

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
            core.info('');
            core.setFailed('Something Went Wrong. Please try again.');
            return;
        }

        core.setOutput('CodeSigner', result);
    } catch (error) {
        if (error instanceof Error) core.setFailed(error.message);
    }
}

run().then();
