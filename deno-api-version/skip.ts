#!/usr/bin/env -S deno run --allow-env --allow-read --allow-write --allow-net
// From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past
//    & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past
// Implementation summary :
//     1. Check if the script has already been completed : check ci-skip file. If file exists, exit, else :
//     2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
//     3. Get last successful jobs of the project
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
// image: jersou/alpine-deno-git-unzip
// variables:
//     GIT_DEPTH: 10000
//     SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
// script:
//     - ./skip.ts || service-A/test1.sh
//     - ./skip.ts || service-A/test2.sh
//     - ./skip.ts || service-A/test3.sh

import {
  bgBlack,
  bgGreen,
  bgRed,
  bgYellow,
} from "https://deno.land/std@0.130.0/fmt/colors.ts";
import {exists} from "https://deno.land/std@0.130.0/fs/exists.ts";

const pageToFetchMax = 5;
const commitToCheckSameRefMax = 2;
const commitToCheckSameJobMax = 100;
const jobToCheckMax = 1000;

const red = (msg: string) => bgRed(bgBlack(msg));
const yellow = (msg: string) => bgYellow(bgBlack(msg));
const green = (msg: string) => bgGreen(bgBlack(msg));
const ciBuildsDir = Deno.env.get("CI_BUILDS_DIR");
const ciProjectDir = Deno.env.get("CI_PROJECT_DIR") || undefined;
const projectPath = ciProjectDir!.startsWith(ciBuildsDir!)
  ? ciProjectDir
  : ciBuildsDir + ciProjectDir!.match(/(^\/[^\/]+)(.*)/)![2]; // remove first part of path
const ciSkipPath = `${projectPath}/ci-skip-${Deno.env.get("CI_PROJECT_ID")}-${
  Deno.env.get("CI_JOB_ID")
}`;
const isVerbose = Deno.env.get("SKIP_CI_VERBOSE") === "true";
const verbose = (msg: string) => isVerbose && console.log(msg);

if (!Deno.env.get("SKIP_IF_TREE_OK_IN_PAST")) {
  red(
    "⚠️ The SKIP_IF_TREE_OK_IN_PAST variable is empty, set the list of paths to check",
  );
  Deno.exit(1);
}
if (!Deno.env.get("API_READ_TOKEN")) {
  red("⚠️ The API_READ_TOKEN variable is empty !");
  Deno.exit(1);
}

if (await exists(ciSkipPath)) {
  const content = (await Deno.readTextFile(ciSkipPath)).trim();
  verbose(`ci-skip file exists, content=${content}`);
  Deno.exit(content === "true" ? 0 : 3);
}

type Job = {
  id: number;
  artifacts_expire_at: string;
};

async function getTree(commit: string) {
  const process = await Deno.run({
    cmd: [
      "git",
      "ls-tree",
      commit,
      "--",
      ...Deno.env.get("SKIP_IF_TREE_OK_IN_PAST")!.split(" "),
    ],
    stdout: "piped",
  });
  if (!(await process.status()).success) {
    throw new Error("Error while 'git ls-tree'");
  }
  return process.output();
}

async function fetchJson(url: string) {
  const resp = await fetch(url);
  if (resp.status !== 200) {
    throw new Error(`Status Code: ${resp.status} !`);
  }
  return resp.json();
}

async function downloadFile(path: string, url: string) {
  verbose(`DownloadFile file`);
  const file = Deno.create(path);
  const resp = await fetch(url);
  if (resp.status === 302 && resp.headers.get("location")) {
    verbose(`→ 302 follow the redirection`);
    const location = resp.headers.get("location");
    await downloadFile(path, location!);
  } else if (resp.status !== 200) {
    console.error(resp);
    throw new Error(`Status Code: ${resp.status} !`);
  } else {
    await resp.body!.pipeTo((await file).writable);
  }
}

async function extractArtifacts(job: Job) {
  console.log(`job ${job.id} artifacts_expire_at: ${job.artifacts_expire_at}`);
  if (job.artifacts_expire_at) {
    verbose(`Extract artifacts of job : ${job.id}`);
    const hasUnzipCmd =
      (await Deno.run({cmd: ["unzip", "-h"]}).status()).success;
    if (!hasUnzipCmd) {
      red("unzip not found, skip artifacts dl/extract.");
      return;
    }
    const artifactsPath = "artifacts.zip";
    console.log(`download artifacts.zip`);
    await downloadFile(
      artifactsPath,
      `${Deno.env.get("CI_API_V4_URL")}/projects/${
        Deno.env.get("CI_PROJECT_ID")
      }/jobs/${job.id}/artifacts?job_token=${Deno.env.get("CI_JOB_TOKEN")}`,
    );
    console.log(`unzip artifacts.zip`);
    const process = await Deno.run({cmd: ["unzip", artifactsPath]});
    await Deno.remove(artifactsPath);
    if (!(await process.status()).success) {
      red("artifacts not found, expired ? → Don't skip");
      await Deno.writeTextFile(ciSkipPath, "false");
      Deno.exit(5);
    }
  }
}

async function exitNotFound() {
  await Deno.writeTextFile(ciSkipPath, "false");
  yellow("❌ tree not found in last success jobs of the project");
  Deno.exit(4);
}

async function main() {
  const currentTree = getTree("HEAD");
  verbose(
    "------------------------------ Current tree : ----------------------------------\n" +
    currentTree +
    "--------------------------------------------------------------------------------",
  );

  let commitCheckedSameRef = 0;
  let commitCheckedSameJob = 0;
  let jobChecked = 0;
  const ciCommitRefName = Deno.env.get("CI_COMMIT_REF_NAME");

  for (let page = 0; page < pageToFetchMax; page++) {
    const projectJobs = await fetchJson(
      `${Deno.env.get("CI_API_V4_URL")}/projects/${
        Deno.env.get("CI_PROJECT_ID")
      }/jobs?scope=success&per_page=1000&page=&private_token=${
        Deno.env.get("API_READ_TOKEN")
      }`,
    );
    for (const job of projectJobs) {
      if (job.name === Deno.env.get("CI_JOB_NAME")) {
        verbose(
          `process job with same name, jobChecked=${jobChecked},` +
          ` commitCheckedSameJob=${commitCheckedSameJob}`,
        );
        try {
          const tree = getTree(job.commit.id);
          verbose(
            "------------------------------     tree :     ----------------------------------\n" +
            tree +
            "--------------------------------------------------------------------------------",
          );
          if (currentTree === tree) {
            await extractArtifacts(job);
            await Deno.writeTextFile(ciSkipPath, "true");
            green(`✅ tree found in job ${job.web_url}`);
            Deno.exit(0);
          }
          if (job.ref === ciCommitRefName) {
            commitCheckedSameRef++;
            verbose(`The job have the same ref name (${commitCheckedSameRef})`);
          }
          commitCheckedSameJob++;
        } catch (_) {
          // ignore
        }
        jobChecked++;
        if (
          jobChecked >= jobToCheckMax ||
          commitCheckedSameJob >= commitToCheckSameJobMax ||
          commitCheckedSameRef >= commitToCheckSameRefMax
        ) {
          verbose("[exit not found] : ");
          verbose(`jobChecked : ${jobChecked} /${jobToCheckMax} `);
          verbose(
            `commitCheckedSameJob : ${commitCheckedSameJob} /${commitToCheckSameJobMax} `,
          );
          verbose(
            `commitCheckedSameRef : ${commitCheckedSameRef} /${commitToCheckSameRefMax} `,
          );
          await exitNotFound();
        }
      }
    }
  }
  await exitNotFound();
}

await main();
