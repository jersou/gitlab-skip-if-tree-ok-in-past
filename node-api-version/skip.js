#!/usr/bin/env node
// From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past
//    & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past
// Implementation summary :
//     1. Check if the script has already been completed : check ci-skip file. If file exists, exit, else :
//     2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
//     3. Get last 1000 successful jobs of the project
//     4. Filter jobs : keep current job only
//     5. For each job :
//         1. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST"
//         2. Check if this "git ls-tree" equals the current HEAD "git ls-tree" (see 2.)
//         3. If the "git ls-tree" are equals, write true in ci-skip file and exit with code 0
//     6. If no job found, write false in ci-skip file and exit with code > 0
//
// ⚠️ Requirements :
//    - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
//    - docker images/gitlab runner need :  git, nodejs, unzip (optional, used to extract artifacts)
//    - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
//    - CI variables changes are not detected
//    - need API_READ_TOKEN (personal access tokens that have read_api scope)
//    - set GIT_DEPTH variable to 1000 or more
//
// Set env var SKIP_CI_VERBOSE=true to enable verbose log
//
// usage in .gitlab-ci.yml file :
//     SERVICE-A:
// stage: test
// image: jersou/alpine-git-nodejs-unzip
// variables:
//     GIT_DEPTH: 10000
//     SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
// script:
//     - ./skip.js || service-A/test1.sh
//     - ./skip.js || service-A/test2.sh
//     - ./skip.js || service-A/test3.sh

const fs = require("fs");
const { spawn, execFileSync } = require("child_process");
const http = require("http");
const https = require("https");
const crypto = require("crypto");

const pageToFetchMax = 5;
const commitToCheckSameRefMax = 2;
const commitToCheckSameJobMax = 100;
const jobToCheckMax = 1000;

const color = (color, msg) => console.error(`\x1b[${color}m  ${msg}  \x1b[0m`);
const red = (msg) => color("1;41;30", msg);
const yellow = (msg) => color("1;43;30", msg);
const green = (msg) => color("1;42;30", msg);
const ciBuildsDir = process.env.CI_BUILDS_DIR;
const ciProjectDir = process.env.CI_PROJECT_DIR;
const projectPath = ciProjectDir.startsWith(ciBuildsDir)
  ? ciProjectDir
  : ciBuildsDir + ciProjectDir.match(/(^\/[^\/]+)(.*)/)[2]; // remove first part of path
const ciSkipPath = `${projectPath}/ci-skip-${process.env.CI_PROJECT_ID}-${process.env.CI_JOB_ID}`;
const isVerbose = process.env.SKIP_CI_VERBOSE === "true";
const verbose = (msg) => isVerbose && console.log(msg);

if (!process.env.SKIP_IF_TREE_OK_IN_PAST) {
  red(
    "⚠️ The SKIP_IF_TREE_OK_IN_PAST variable is empty, set the list of paths to check"
  );
  process.exit(1);
}
if (!process.env.API_READ_TOKEN) {
  red("⚠️ The API_READ_TOKEN variable is empty !");
  process.exit(1);
}

if (fs.existsSync(ciSkipPath)) {
  const content = fs.readFileSync(ciSkipPath, "utf8").trim();
  verbose(`ci-skip file exists, content=${content}`);
  process.exit(content === "true" ? 0 : 3);
}

function getTree(commit) {
  return execFileSync(
    "git",
    [
      "ls-tree",
      commit,
      "--",
      ...process.env.SKIP_IF_TREE_OK_IN_PAST.split(" "),
    ],
    { stdio: ["pipe", "pipe", null] }
  ).toString();
}

function fetchJson(url) {
  return new Promise((resolve, reject) => {
    let client = url.match(/^https/) ? https : http;
    client
      .get(url, (resp) => {
        if (resp.statusCode !== 200) {
          reject(`Status Code: ${resp.statusCode} !`);
        }
        let data = "";
        resp.on("data", (chunk) => (data += chunk));
        resp.on("end", () => resolve(JSON.parse(data)));
      })
      .on("error", (err) => reject(err));
  });
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

async function extractArtifacts(job) {
  console.log(`job ${job.id} artifacts_expire_at: ${job.artifacts_expire_at}`);
  if (job.artifacts_expire_at) {
    verbose(`Extract artifacts of job : ${job.Id}`);
    try {
      execFileSync("unzip", ["-h"]);
    } catch (error) {
      red("unzip not found, skip artifacts dl/extract.");
      return;
    }
    try {
      const artifactsPath = "artifacts.zip";
      console.log(`download artifacts.zip`);
      await downloadFile(
        artifactsPath,
        `${process.env.CI_API_V4_URL}/projects/${process.env.CI_PROJECT_ID}/jobs/${job.id}/artifacts?job_token=${process.env.CI_JOB_TOKEN}`
      );
      console.log(`unzip artifacts.zip`);
      execFileSync("unzip", [artifactsPath]);
      fs.unlinkSync(artifactsPath);
    } catch (error) {
      console.error(error);
      red("artifacts not found, expired ? → Don't skip");
      fs.writeFileSync(ciSkipPath, "false");
      process.exit(5);
    }
  }
}

function exitNotFound() {
  fs.writeFileSync(ciSkipPath, "false");
  yellow("❌ tree not found in last 1000 success jobs of the project");
  process.exit(4);
}

async function main() {
  const currentTree = getTree("HEAD");
  verbose(
    "------------------------------ Current tree : ----------------------------------\n" +
      currentTree +
      "--------------------------------------------------------------------------------"
  );

  let commitCheckedSameRef = 0;
  let commitCheckedSameJob = 0;
  let jobChecked = 0;
  const ciCommitRefName = process.envCI_COMMIT_REF_NAME;

  for (let page = 0; page < pageToFetchMax; page++) {
    const projectJobs = await fetchJson(
      `${process.env.CI_API_V4_URL}/projects/${process.env.CI_PROJECT_ID}/jobs?scope=success&per_page=1000&page=&private_token=${process.env.API_READ_TOKEN}`
    );
    for (const job of projectJobs) {
      if (job.name === process.env.CI_JOB_NAME) {
        verbose(
          `process job with same name, jobChecked=${jobChecked},` +
            ` commitCheckedSameJob=${commitCheckedSameJob}`
        );
        try {
          const tree = getTree(job.commit.id);
          verbose(
            "------------------------------     tree :     ----------------------------------\n" +
              tree +
              "--------------------------------------------------------------------------------"
          );
          if (currentTree === tree) {
            await extractArtifacts(job);
            fs.writeFileSync(ciSkipPath, "true");
            green(`✅ tree found in job ${job.web_url}`);
            process.exit(0);
          }
          if (job.ref === ciCommitRefName) {
            commitCheckedSameRef++;
            verbose(`The job have the same ref name (${commitCheckedSameRef})`);
          }
          commitCheckedSameJob++;
        } catch (_) {}
        jobChecked++;
        if (
          jobChecked >= jobToCheckMax ||
          commitCheckedSameJob >= commitToCheckSameJobMax ||
          commitCheckedSameRef >= commitToCheckSameRefMax
        ) {
          verbose("[exit not found] : ");
          verbose(`jobChecked : ${jobChecked} /${jobToCheckMax} `);
          verbose(
            `commitCheckedSameJob : ${commitCheckedSameJob} /${commitToCheckSameJobMax} `
          );
          verbose(
            `commitCheckedSameRef : ${commitCheckedSameRef} /${commitToCheckSameRefMax} `
          );
          exitNotFound();
        }
      }
    }
  }
  exitNotFound();
}

main().then();
