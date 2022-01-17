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
	"strconv"
	"strings"
)

func printHelp() {
	println(`Help :
From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past 
   & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past
Implementation summary :
    1. Check if the script has already been completed : check /tmp/ci-skip. If file exists, exit, else :
    2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD
    3. Get last successful jobs of the project
    4. Filter jobs : keep current job only
    5. For each job :
        1. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST"
        2. Check if this "git ls-tree" equals the current HEAD "git ls-tree" (see 2.)
        3. If the "git ls-tree" are equals, write true in /tmp/ci-skip and exit with code 0
    6. If no job found, write false in /tmp/ci-skip and exit with code > 0

⚠️  Requirements :
   - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
   - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
   - CI variables changes are not detected
   - need API_READ_TOKEN (personal access tokens that have read_api scope)
   - set GIT_DEPTH variable to 1000 or more

Set env var SKIP_CI_VERBOSE=true to enable verbose log

Usage in .gitlab-ci.yml file :
  SERVICE-A:
    stage: test
    image: alpine
    variables:
      GIT_DEPTH: 1000
    SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
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

func verbose(msg string) {
	if os.Getenv("SKIP_CI_VERBOSE") == "true" {
		println(msg)
	}
}

func red(msg string) {
	fmt.Println(string("\033[1;41;30m"), " ", msg, " ", string("\033[0m"))
}
func yellow(msg string) {
	fmt.Println("\033[1;43;30m", " ", msg, " ", "\033[0m")
}
func green(msg string) {
	fmt.Println("\033[1;42;30m", " ", msg, " ", "\033[0m")
}

func exitIfError(err error) {
	if err != nil {
		fmt.Printf("\x1b[31;1m%s\x1b[0m\n", fmt.Sprintf("error: %s", err))
		red(fmt.Sprintf("error: %s", err))
		_ = os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
		os.Exit(1)
	}
}

func getTreeOfPaths(repository *git.Repository, hash plumbing.Hash, paths []string) (string, error) {
	commit, err := repository.CommitObject(hash)
	if err != nil {
		return "", err
	}
	tree, err := repository.TreeObject(commit.TreeHash)
	if err != nil {
		return "", err
	}
	entries := ""
	for _, path := range paths {
		entry, err := tree.FindEntry(string(path))
		if err != nil {
			return "", err
		}
		entries += entry.Hash.String() + " " + string(path) + "\n"
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
	exitIfError(err)
	if res.Body != nil {
		defer res.Body.Close()
	}
	var jobs []Job
	err = json.NewDecoder(res.Body).Decode(&jobs)
	exitIfError(err)
	return jobs
}

func extractArchive(archivePath string, outputPath string) {
	verbose("Extract archive : " + archivePath)
	archive, err := zip.OpenReader(archivePath)
	exitIfError(err)
	defer archive.Close()
	for _, f := range archive.File {
		verbose("Extract archive file : " + f.Name)
		filePath := filepath.Join(outputPath, f.Name)
		fmt.Println("unzipping file ", filePath)
		if f.FileInfo().IsDir() {
			err = os.MkdirAll(filePath, os.ModePerm)
			exitIfError(err)
			continue
		}
		err := os.MkdirAll(filepath.Dir(filePath), os.ModePerm)
		exitIfError(err)
		dstFile, err := os.OpenFile(filePath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, f.Mode())
		exitIfError(err)
		fileInArchive, err := f.Open()
		exitIfError(err)
		_, err = io.Copy(dstFile, fileInArchive)
		exitIfError(err)
		err = dstFile.Close()
		exitIfError(err)
		err = fileInArchive.Close()
		exitIfError(err)
	}
}

func downloadFile(filepath string, url string) {
	verbose("DownloadFile file : " + url)
	resp, err := http.Get(url)
	exitIfError(err)
	defer resp.Body.Close()
	out, err := os.Create(filepath)
	exitIfError(err)
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	exitIfError(err)
}

func extractArtifacts(job Job) {
	verbose("Extract artifacts of job : " + strconv.Itoa(job.Id))
	println("job", job.Id, "artifacts_expire_at:", job.Artifacts_expire_at)
	if job.Artifacts_expire_at != "" {
		artifactsPath := "artifacts.zip"
		println("Download", artifactsPath)
		url := os.Getenv("CI_API_V4_URL") +
			"/projects/" + os.Getenv("CI_PROJECT_ID") +
			"/jobs/" + string(job.Id) +
			"/artifacts?job_token=" + os.Getenv("CI_JOB_TOKEN")
		downloadFile(artifactsPath, url)
		println("unzip", artifactsPath)
		extractArchive(artifactsPath, "./")
		verbose("Remove file : " + artifactsPath)
		_ = os.Remove(artifactsPath)
	}
}

func getCiSkipPath() string {
	return "/tmp/ci-skip-" + os.Getenv("CI_PROJECT_ID") + "-" + os.Getenv("CI_JOB_ID}")
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
	if os.Getenv("API_READ_TOKEN") == "" {
		red("Error : API_READ_TOKEN is empty")
		printHelp()
		os.Exit(2)
	}
	ciSkipPath := getCiSkipPath()
	if _, err := os.Stat(ciSkipPath); err == nil {
		content, err := os.ReadFile(ciSkipPath)
		verbose("ci-skip file exists, content=" + string(content))
		exitIfError(err)
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
	exitIfError(err)
	head, err := repository.Head()
	exitIfError(err)
	paths := strings.Split(os.Getenv("SKIP_IF_TREE_OK_IN_PAST"), " ")
	currentTree, err := getTreeOfPaths(repository, head.Hash(), paths)
	exitIfError(err)
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
					exitIfError(err)
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
