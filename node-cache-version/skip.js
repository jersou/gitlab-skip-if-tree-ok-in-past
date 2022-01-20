#!/usr/bin/env node
// From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past
//    & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past
// Implementation summary :
//     1. Check if the script has already been completed : check "ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
//     2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
//     3. Check if the SHA-1 is present in the "ci_ok_history"
//     4. If found, write true in "ci-skip", download and extract the artifact of the found job and exit with code 0
//     5. If not found, write false in "ci-skip", append the SHA-1:CI_JOB_ID to "ci_ok_history" and exit 2
//
// ⚠️ Requirements :
//    - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
//    - a cache must be defined to keep the ci_ok_history file
//    - docker images/gitlab runner need : git, nodejs, unzip (optional, used to extract artifacts)
//    - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
//    - CI variables changes are not detected. It could be by adding the variables to the tree used to generate the SHA-1.
//
// Usage :
//   Set env var SKIP_CI_VERBOSE=true to enable verbose log
//   Set env var SKIP_CI_NO_ARTIFACT=true to disable artifacts download & extract
//   Set env var SKIP_CI_VALUE=false to disable skip
//   in .gitlab-ci.yml file :
// SERVICE-A:
//   stage: test
//   image: jersou/alpine-git-nodejs-unzip
//   cache:
//     - key: "${CI_PROJECT_NAMESPACE}__${CI_PROJECT_NAME}__${CI_JOB_NAME}__ci_ok_history"
//       policy: pull-push
//       paths:
//           - ci_ok_history
//   variables:
//       SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.js
//   script:
//       - ./skip.js || service-A/test1.sh
//       - ./skip.js || service-A/test2.sh
//       - ./skip.js || service-A/test3.sh

const fs = require("fs");
const { execFileSync } = require("child_process");
const http = require("http");
const https = require("https");
const crypto = require("crypto");
const HISTORY_MAX = 500;
const color = (color, msg) => console.error(`\x1b[${color}m  ${msg}  \x1b[0m`);
const red = (msg) => color("1;41;30", msg);
const yellow = (msg) => color("1;43;30", msg);
const green = (msg) => color("1;42;30", msg);
const isVerbose = process.env.SKIP_CI_VERBOSE === "true";
const verbose = (msg) => isVerbose && console.log(msg);

const SkipIfTreeOkInPast = process.env.SKIP_IF_TREE_OK_IN_PAST;
const ciBuildsDir = process.env.CI_BUILDS_DIR;
const ciProjectDir = process.env.CI_PROJECT_DIR;
const projectPath = ciProjectDir.startsWith(ciBuildsDir)
  ? ciProjectDir
  : ciBuildsDir + ciProjectDir.match(/(^\/[^\/]+)(.*)/)[2]; // remove first part of path
const ciSkipPath = `${projectPath}/ci-skip-${process.env.CI_PROJECT_ID}-${process.env.CI_JOB_ID}`;
const ciHistoryPath = `${projectPath}/ci_ok_history`;

verbose(`ciBuildsDir=${ciBuildsDir}`);
verbose(`ciProjectDir=${ciProjectDir}`);
verbose(`projectPath=${projectPath}`);

function getTree(commit) {
  return execFileSync(
    "git",
    ["ls-tree", commit, "--", ...SkipIfTreeOkInPast.split(" ")],
    { stdio: ["pipe", "pipe", null] }
  ).toString();
}

function downloadFile(path, url) {
  return new Promise((resolve, reject) => {
    verbose(`DownloadFile file`);
    const file = fs.createWriteStream(path);
    let client = url.match(/^https/) ? https : http;
    client
      .get(url, (res) => {
        if (res.statusCode === 302 && res.headers.location) {
          verbose(`→ 302 follow the redirection`);
          const location = res.headers.location;
          downloadFile(path, location).then(resolve);
        } else if (res.statusCode !== 200) {
          console.error(res);
          reject(`Status Code: ${res.statusCode} !`);
        } else {
          res.pipe(file);
          res.on("end", () => resolve());
        }
      })
      .on("error", (err) => reject(err));
  });
}

async function extractArtifacts(jobId) {
  if (process.env.SKIP_CI_NO_ARTIFACT !== "true") {
    verbose(`Extract artifacts of job : ${jobId}`);
    try {
      execFileSync("unzip", ["-h"]);
    } catch (error) {
      red("unzip not found, skip artifacts dl/extract.");
      return;
    }
    try {
      const artifactsPath = "artifacts.zip";
      console.log(`download ${artifactsPath}`);
      await downloadFile(
        artifactsPath,
        `${process.env.CI_API_V4_URL}/projects/${process.env.CI_PROJECT_ID}/jobs/${jobId}/artifacts?job_token=${process.env.CI_JOB_TOKEN}`
      );
      console.log(`unzip artifacts.zip`);
      execFileSync("unzip", [artifactsPath]);
      fs.unlinkSync(artifactsPath);
    } catch (error) {
      verbose(error);
      verbose("Artifacts not found.");
    }
  }
}

function initCheck() {
  if (process.env.SKIP_CI_VALUE) {
    fs.writeFileSync(ciSkipPath, process.env.SKIP_CI_VALUE);
    verbose(`${SKIP_CI_VALUE}=${process.env.SKIP_CI_VALUE}`);
    process.exit(process.env.SKIP_CI_VALUE === "true" ? 0 : 3);
  }
  if (fs.existsSync(ciSkipPath)) {
    const content = fs.readFileSync(ciSkipPath, "utf8").trim();
    verbose(`ci-skip file exists, content=${content}`);
    process.exit(content === "true" ? 0 : 4);
  }
  if (!SkipIfTreeOkInPast) {
    red("⚠️ The SKIP_IF_TREE_OK_IN_PAST variable is empty");
    process.exit(1);
  }
}

function fileExists(path) {
  try {
    fs.accessSync(path, fs.constants.R_OK);
    return true;
  } catch (e) {
    return false;
  }
}

function getSha(str) {
  const shaSum = crypto.createHash("sha1");
  shaSum.update(str);
  return shaSum.digest("base64");
}

async function main() {
  // 1. Check if the script has already been completed : check "ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
  initCheck();
  // 2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
  const currentTree = getTree("HEAD");
  const currentTreeSha = getSha(currentTree);
  verbose(
    "------------------------------ Current tree : ----------------------------------\n" +
      currentTree +
      "--------------------------------------------------------------------------------"
  );
  verbose(`currentTreeSha=${currentTreeSha}`);
  if (!currentTree || !currentTreeSha) {
    red("Tree empty !");
    fs.writeFileSync(ciSkipPath, "false");
    process.exit(5);
  }

  // 3. Check if the SHA-1 is present in the history file
  const historyFileExist = fileExists(ciHistoryPath);
  const history = historyFileExist
    ? fs.readFileSync(ciHistoryPath, "utf8").trim().split("\n")
    : [];
  isVerbose && verbose("history=\n" + history.join("\n"));

  const foundLine = history.find((line) => line.startsWith(currentTreeSha));
  verbose(`foundLine=${foundLine}`);

  if (foundLine) {
    // 4. If found, write true in "ci-skip", download and extract the artifact of the found job and exit with code 0
    fs.writeFileSync(ciSkipPath, "true");
    const jobId = foundLine.split(":")[1];
    green(`✅ tree found in job ${jobId}`);
    await extractArtifacts(jobId);
    process.exit(0);
  } else {
    // 5. If not found, write false in "ci-skip", append the SHA-1:CI_JOB_ID to "ci_ok_history" and exit 2
    fs.writeFileSync(ciSkipPath, "false");
    verbose(
      `Append <${currentTreeSha}:${process.env.CI_JOB_ID}> to file ${ciHistoryPath}`
    );
    fs.writeFileSync(
      ciHistoryPath,
      `${currentTreeSha}:${process.env.CI_JOB_ID}\n` +
        history.slice(0, HISTORY_MAX).join("\n")
    );
    yellow("❌ tree not found in history");
    process.exit(2);
  }
}

main().then();
