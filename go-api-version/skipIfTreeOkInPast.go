package main

import (
	"archive/zip"
	"encoding/json"
	"fmt"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strconv"
	"strings"
	"time"
)

func printHelp() {
	println(`Help :
From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past 
   & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past

Version : go-api-version

Implementation summary :
    1. Check if the script has already been completed : check ci-skip file. If file exists, exit, else :
    2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
    3. Get last successful jobs of the project
    4. Filter jobs : keep current job only
    5. For each job :
        1. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST"
        2. Check if this "git ls-tree" equals the current HEAD "git ls-tree" (see 2.)
        3. If the "git ls-tree" are equals, write true in ci-skip file and exit with code 0
    6. If no job found, write false in ci-skip file and exit with code > 0

⚠️  Requirements :
   - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
   - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
   - CI variables changes are not detected
   - need API_READ_TOKEN (personal access tokens that have read_api scope)
   - set GIT_DEPTH variable to 1000 or more

Set the env var SKIP_CI_VERBOSE=true to enable verbose log

Set the env var FAIL_IF_ARTIFACTS_EXPIRED=true to fail if the artifact is expired

Usage in .gitlab-ci.yml file :
  SERVICE-A:
    stage: test
    image: alpine
    variables:
        GIT_DEPTH: 1000
        SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip-if-tree-ok-in-past
    script:
        - ./skip-if-tree-ok-in-past || service-A/test1.sh
        - ./skip-if-tree-ok-in-past || service-A/test2.sh
        - ./skip-if-tree-ok-in-past || service-A/test3.sh
`)
}

const pageToFetchMax = 5
const commitToCheckSameRefMax = 2
const commitToCheckSameJobMax = 100
const jobToCheckMax = 1000

type Job struct {
	Id                  int
	Name                string
	Ref                 string
	Web_url             string
	Artifacts_expire_at string
	Commit              struct {
		Id string
	}
}

// print msg only if SKIP_CI_VERBOSE env var == true
func verbose(msg string) {
	if os.Getenv("SKIP_CI_VERBOSE") == "true" {
		println(msg)
	}
}

func red(msg string) {
	fmt.Println("\033[1;41;30m  ", msg, "  \033[0m")
}
func yellow(msg string) {
	fmt.Println("\033[1;43;30m  ", msg, "  \033[0m")
}
func green(msg string) {
	fmt.Println("\033[1;42;30m  ", msg, "  \033[0m")
}

func exitIfError(err error, msg string) {
	if err != nil {
		red(fmt.Sprintf("error: %s", err))
		exitError(msg)
	}
}
func exitError(msg string) {
	red("exitIfError : " + msg)
	_ = os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
	os.Exit(1)
}

func getTreeOfPaths(repository *git.Repository, hash plumbing.Hash, paths []string) (string, error) {
	verbose("getTreeOfPaths: hash=" + hash.String())
	commit, err := repository.CommitObject(hash)
	if err != nil {
		verbose("error: repository.CommitObject(hash) : " + hash.String())
		return "", err
	}
	tree, err := repository.TreeObject(commit.TreeHash)
	if err != nil {
		verbose("error: repository.TreeObject(commit.TreeHash) : " + commit.TreeHash.String())
		return "", err
	}
	entries := ""
	for _, path := range paths {
		entry, err := tree.FindEntry(strings.TrimSuffix(path, "/"))
		if err != nil {
			verbose("error: tree.FindEntry(string(path)) : " + path)
			return "", err
		}
		entries += entry.Hash.String() + " " + path + "\n"
	}
	return entries, nil
}

func getProjectJobs(page int) []Job {
	verbose("GET /jobs?scope=success&per_page=100&page=" + strconv.Itoa(page))
	url := os.Getenv("CI_API_V4_URL") +
		"/projects/" + os.Getenv("CI_PROJECT_ID") +
		"/jobs?scope=success&per_page=100&page=" + strconv.Itoa(page) +
		"&private_token=" + os.Getenv("API_READ_TOKEN")
	res, err := http.Get(url)
	exitIfError(err, "getProjectJobs::http.Get")
	if res.Body != nil {
		defer res.Body.Close()
	}
	var jobs []Job
	err = json.NewDecoder(res.Body).Decode(&jobs)
	exitIfError(err, "json.NewDecoder(res.Body).Decode(&jobs)")
	return jobs
}

func extractArchive(archivePath string, outputPath string) {
	verbose("Extract archive : " + archivePath)
	archive, err := zip.OpenReader(archivePath)
	exitIfError(err, "extractArchive::zip.OpenReader")
	defer archive.Close()
	for _, f := range archive.File {
		verbose("Extract archive file : " + f.Name)
		filePath := filepath.Join(outputPath, f.Name)
		fmt.Println("unzipping file ", filePath)
		if f.FileInfo().IsDir() {
			err = os.MkdirAll(filePath, os.ModePerm)
			exitIfError(err, "f.FileInfo().IsDir() extractArchive::os.MkdirAll")
			continue
		}
		err := os.MkdirAll(filepath.Dir(filePath), os.ModePerm)
		exitIfError(err, "extractArchive::os.MkdirAll")
		dstFile, err := os.OpenFile(filePath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, f.Mode())
		exitIfError(err, "extractArchive::os.OpenFile")
		fileInArchive, err := f.Open()
		exitIfError(err, "extractArchive::f.Open")
		_, err = io.Copy(dstFile, fileInArchive)
		exitIfError(err, "extractArchive::io.Copy")
		err = dstFile.Close()
		exitIfError(err, "extractArchive::dstFile.Close()")
		err = fileInArchive.Close()
		exitIfError(err, "extractArchive::fileInArchive.Close()")
	}
}

func downloadFile(filepath string, url string) {
	verbose("DownloadFile file : " + url)
	resp, err := http.Get(url)
	exitIfError(err, "downloadFile::http.Get(url)")
	defer resp.Body.Close()
	out, err := os.Create(filepath)
	exitIfError(err, "downloadFile::os.Create(filepath)")
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	exitIfError(err, "downloadFile::io.Copy(out, resp.Body)")
}

func extractArtifacts(job Job) {
	verbose("Extract artifacts of job : " + strconv.Itoa(job.Id))
	println("job", job.Id, "artifacts_expire_at:", job.Artifacts_expire_at)
	if job.Artifacts_expire_at != "" {
		parseExpireAt, err := time.Parse(time.RFC3339, job.Artifacts_expire_at)
	    exitIfError(err, "expire_at parse error")
		isExpired := parseExpireAt.Before(time.Now())
		if isExpired {
			if os.Getenv("FAIL_IF_ARTIFACTS_EXPIRED") == "true" {
				exitError( "Artifact is expired")
			} else {
				yellow("Artifact is expired, we ignore it")
			}
		} else {
			artifactsPath := "artifacts.zip"
			println("Download", artifactsPath)
			url := os.Getenv("CI_API_V4_URL") +
				"/projects/" + os.Getenv("CI_PROJECT_ID") +
				"/jobs/" + strconv.Itoa(job.Id) +
				"/artifacts?job_token=" + os.Getenv("CI_JOB_TOKEN")
			downloadFile(artifactsPath, url)
			println("unzip", artifactsPath)
			extractArchive(artifactsPath, "./")
			verbose("Remove file : " + artifactsPath)
			_ = os.Remove(artifactsPath)
		}
	}
}

func getProjectPath() string {
	ciBuildsDir := os.Getenv("CI_BUILDS_DIR")
	ciProjectDir := os.Getenv("CI_PROJECT_DIR")
	if strings.HasPrefix(ciProjectDir, ciBuildsDir) {
		return ciProjectDir + "/"
	} else {
		reg := regexp.MustCompile(`/([^/]+)(.*)`)
		res := reg.ReplaceAllString(ciProjectDir, "${2}")
		return ciBuildsDir + res + "/"
	}
}

func getCiSkipPath() string {
	return getProjectPath() + "ci-skip-" + os.Getenv("CI_PROJECT_ID") + "-" + os.Getenv("CI_JOB_ID")
}

func exitNotFound() {
	_ = os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
	yellow("❌ tree not found in last jobs of the project")
	os.Exit(4)
}

func initCheck() {
	if len(os.Args) > 1 {
		printHelp()
		os.Exit(1)
	}
	if os.Getenv("SKIP_IF_TREE_OK_IN_PAST") == "" {
		red("Error : SKIP_IF_TREE_OK_IN_PAST is empty")
		printHelp()
		os.Exit(1)
	}
	verbose("SKIP_IF_TREE_OK_IN_PAST=" + os.Getenv("SKIP_IF_TREE_OK_IN_PAST"))
	if os.Getenv("API_READ_TOKEN") == "" {
		red("Error : API_READ_TOKEN is empty")
		printHelp()
		os.Exit(2)
	}
	ciSkipPath := getCiSkipPath()
	verbose("ciSkipPath=" + ciSkipPath)
	if _, err := os.Stat(ciSkipPath); err == nil {
		content, err := os.ReadFile(ciSkipPath)
		verbose("ci-skip file exists, content=" + string(content))
		exitIfError(err, "initCheck::os.ReadFile(ciSkipPath)")
		if string(content) == "true" {
			os.Exit(0)
		} else {
			os.Exit(3)
		}
	}
}

func main() {
	initCheck()
	ciJobName := os.Getenv("CI_JOB_NAME")
	ciCommitRefName := os.Getenv("CI_COMMIT_REF_NAME")
	repository, err := git.PlainOpen(".")
	exitIfError(err, "main::git.PlainOpen(\".\")")
	head, err := repository.Head()
	exitIfError(err, "main::repository.Head()")
	paths := strings.Split(os.Getenv("SKIP_IF_TREE_OK_IN_PAST"), " ")
	currentTree, err := getTreeOfPaths(repository, head.Hash(), paths)
	exitIfError(err, "main::getTreeOfPaths(repository, head.Hash(), paths)")
	verbose("------------------------------ Current tree : ----------------------------------\n" +
		currentTree + "--------------------------------------------------------------------------------")

	commitCheckedSameRef := 0
	commitCheckedSameJob := 0
	jobChecked := 0

	for page := 1; page <= pageToFetchMax; page++ {
		verbose("process page " + strconv.Itoa(page))
		jobs := getProjectJobs(page)
		for _, job := range jobs {
			if job.Name == ciJobName {
				verbose("process job with same name, jobChecked=" + strconv.Itoa(jobChecked) +
					", commitCheckedSameJob=" + strconv.Itoa(commitCheckedSameJob))
				tree, err := getTreeOfPaths(repository, plumbing.NewHash(job.Commit.Id), paths)
				verbose("------------------------------     tree :     ----------------------------------\n" +
					tree + "--------------------------------------------------------------------------------")
				if err == nil && currentTree == tree {
					extractArtifacts(job)
					err := os.WriteFile(getCiSkipPath(), []byte("true"), 0644)
					exitIfError(err, "main::os.WriteFile")
					green("✅ tree found in job " + job.Web_url)
					os.Exit(0)
				}
				if job.Ref == ciCommitRefName {
					commitCheckedSameRef++
					verbose("The job have the same ref name (" + strconv.Itoa(commitCheckedSameRef) + ")")
				}
				commitCheckedSameJob++
			}
			jobChecked++
			if (jobChecked >= jobToCheckMax) ||
				commitCheckedSameJob >= commitToCheckSameJobMax ||
				commitCheckedSameRef >= commitToCheckSameRefMax {
				verbose("[exit not found] : ")
				verbose(fmt.Sprint("jobChecked : %d /%d ", jobChecked, jobToCheckMax))
				verbose(fmt.Sprint("commitCheckedSameJob : %d /%d ", commitCheckedSameJob, commitToCheckSameJobMax))
				verbose(fmt.Sprint("commitCheckedSameRef : %d /%d ", commitCheckedSameRef, commitToCheckSameRefMax))
				exitNotFound()
			}
		}
	}
	exitNotFound()
}
