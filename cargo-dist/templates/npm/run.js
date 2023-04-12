#!/usr/bin/env node

const { run, install: maybeInstall } = require("./binary");
maybeInstall(true).then(run);