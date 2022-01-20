package main

import (
	"archive/zip"
	"crypto/sha1"
	"encoding/base64"
	"fmt"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"
)

func printHelp() {
	println(`Help :
From https://gitlab.com/jersou/gitlab-skip-if-tree-ok-in-past 
   & https://github.com/jersou/gitlab-skip-if-tree-ok-in-past

Version : go-cache-version

Implementation summary :
     1. Check if the script has already been completed : check "ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
     2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
     3. Check if the SHA-1 is present in the "ci_ok_history"
     4. If found, write true in "ci-skip", download and extract the artifact of the found job and exit with code 0
     5. If not found, write false in "ci-skip", append the SHA-1:CI_JOB_ID to "ci_ok_history" and exit 2

⚠️  Requirements :
   - the variable SKIP_IF_TREE_OK_IN_PAST must contain the paths used by the job
   - a cache must be defined to keep the ci_ok_history file
   - if the nested jobs of current uses the dependencies key with current, the dependencies files need to be in an artifact
   - CI variables changes are not detected. It could be by adding the variables to the tree used to generate the SHA-1.

Set env var SKIP_CI_VERBOSE=true to enable verbose log
Set env var SKIP_CI_NO_ARTIFACT=true to disable artifacts download & extract
Set env var SKIP_CI_VALUE=false to disable skip

Usage in .gitlab-ci.yml file :
  SERVICE-A:
    stage: test
    image: alpine
    cache:
      - key: "${CI_PROJECT_NAMESPACE}__${CI_PROJECT_NAME}__${CI_JOB_NAME}__ci_ok_history"
        policy: pull-push
        paths:
            - ci_ok_history
    variables:
        SKIP_IF_TREE_OK_IN_PAST: service-A LIB-1 .gitlab-ci.yml skip.sh
    script:
        - ./skip-if-tree-ok-in-past || service-A/test1.sh
        - ./skip-if-tree-ok-in-past || service-A/test2.sh
        - ./skip-if-tree-ok-in-past || service-A/test3.sh
`)
}
func isVerbose() bool {
	return os.Getenv("SKIP_CI_VERBOSE") == "true"
}
func verbose(msg string) {
	if isVerbose() {
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

func getSha(str string) string {
	hash := sha1.New()
	hash.Write([]byte(str))
	return base64.StdEncoding.EncodeToString(hash.Sum(nil))
}

func extractArchive(archivePath string, outputPath string) error {
	verbose("Extract archive : " + archivePath)
	archive, err := zip.OpenReader(archivePath)
	if err != nil {
		verbose("ERROR: extractArchive::zip.OpenReader")
		return err
	}
	defer archive.Close()
	for _, f := range archive.File {
		verbose("Extract archive file : " + f.Name)
		filePath := filepath.Join(outputPath, f.Name)
		fmt.Println("unzipping file ", filePath)
		if f.FileInfo().IsDir() {
			err = os.MkdirAll(filePath, os.ModePerm)
			if err != nil {
				verbose("ERROR: f.FileInfo().IsDir() extractArchive::os.MkdirAll")
				return err
			}
			continue
		}
		err := os.MkdirAll(filepath.Dir(filePath), os.ModePerm)
		if err != nil {
			verbose("ERROR: extractArchive::os.MkdirAll")
			return err
		}
		dstFile, err := os.OpenFile(filePath, os.O_WRONLY|os.O_CREATE|os.O_TRUNC, f.Mode())
		if err != nil {
			verbose("ERROR: extractArchive::os.OpenFile")
			return err
		}
		fileInArchive, err := f.Open()
		if err != nil {
			verbose("ERROR: extractArchive::f.Open")
			return err
		}
		_, err = io.Copy(dstFile, fileInArchive)
		if err != nil {
			verbose("ERROR: extractArchive::io.Copy")
			return err
		}
		err = dstFile.Close()
		if err != nil {
			verbose("ERROR: extractArchive::dstFile.Close()")
			return err
		}
		err = fileInArchive.Close()
		if err != nil {
			verbose("ERROR: extractArchive::fileInArchive.Close()")
			return err
		}
	}
	return nil
}

func downloadFile(filepath string, url string) error {
	verbose("DownloadFile file : " + url)
	resp, err := http.Get(url)
	if err != nil {
		verbose("ERROR: downloadFile::http.Get(url)")
		return err
	}
	defer resp.Body.Close()
	out, err := os.Create(filepath)
	if err != nil {
		verbose("ERROR: downloadFile::os.Create(filepath)")
		return err
	}
	defer out.Close()
	_, err = io.Copy(out, resp.Body)
	if err != nil {
		verbose("ERROR: downloadFile::io.Copy(out, resp.Body)")
		return err
	}
	return nil
}

func extractArtifacts(jobId string) error {
	verbose("Extract artifacts of job : " + jobId)
	artifactsPath := "artifacts.zip"
	verbose("Download " + artifactsPath)
	url := os.Getenv("CI_API_V4_URL") +
		"/projects/" + os.Getenv("CI_PROJECT_ID") +
		"/jobs/" + jobId +
		"/artifacts?job_token=" + os.Getenv("CI_JOB_TOKEN")
	err := downloadFile(artifactsPath, url)
	if err != nil {
		verbose("ERROR: downloadFile(artifactsPath, url)")
		return err
	}
	verbose("unzip " + artifactsPath)
	err = extractArchive(artifactsPath, "./")
	if err != nil {
		verbose("ERROR: extractArchive(artifactsPath, \"./\")")
		return err
	}
	verbose("Remove file : " + artifactsPath)
	_ = os.Remove(artifactsPath)
	return nil
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

func initCheck() error {
	if len(os.Args) > 1 {
		printHelp()
		os.Exit(1)
	}
	if os.Getenv("SKIP_CI_VALUE") != "" {
		err := os.WriteFile(getCiSkipPath(), []byte(os.Getenv("SKIP_CI_VALUE")), 0644)
		if err != nil {
			return err
		}
		verbose(`${SKIP_CI_VALUE}=${process.env.SKIP_CI_VALUE}`)
		if os.Getenv("SKIP_CI_VALUE") == "true" {
			os.Exit(0)
		} else {
			os.Exit(3)
		}
	}

	ciSkipPath := getCiSkipPath()
	verbose("ciSkipPath=" + ciSkipPath)
	if fileExists(ciSkipPath) {
		content, err := os.ReadFile(ciSkipPath)
		verbose("ci-skip file exists, content=" + string(content))
		if err != nil {
			verbose("ERROR: initCheck::os.ReadFile(ciSkipPath)")
			return err
		}
		if string(content) == "true" {
			os.Exit(0)
		} else {
			os.Exit(3)
		}
	}
	if os.Getenv("SKIP_IF_TREE_OK_IN_PAST") == "" {
		red("Error : SKIP_IF_TREE_OK_IN_PAST is empty")
		printHelp()
		os.Exit(1)
	}
	verbose("SKIP_IF_TREE_OK_IN_PAST=" + os.Getenv("SKIP_IF_TREE_OK_IN_PAST"))
	return nil
}

func exitIfError(err error, msg string) {
	if err != nil {
		red("exitIfError : " + msg)
		red(fmt.Sprintf("error: %s", err))
		_ = os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
		os.Exit(1)
	}
}

func fileExists(path string) bool {
	_, err := os.Stat(path)
	return err == nil
}

func main() {
	HISTORY_MAX := 500
	// 1. Check if the script has already been completed : check "ci-skip". If file exists: exit 0 if the content == true, otherwise exit 1
	err := initCheck()
	exitIfError(err, "initCheck")
	// 2. Get the "git ls-tree" of the tree "$SKIP_IF_TREE_OK_IN_PAST" of the current HEAD and generate SHA-1 of this output
	repository, err := git.PlainOpen(".")
	exitIfError(err, "main::git.PlainOpen(\".\")")
	head, err := repository.Head()
	exitIfError(err, "main::repository.Head()")
	paths := strings.Split(os.Getenv("SKIP_IF_TREE_OK_IN_PAST"), " ")
	currentTree, err := getTreeOfPaths(repository, head.Hash(), paths)
	exitIfError(err, "main::getTreeOfPaths(repository, head.Hash(), paths)")
	currentTreeSha := getSha(currentTree)
	verbose("------------------------------ Current tree : ----------------------------------\n" +
		currentTree + "--------------------------------------------------------------------------------")
	verbose(`currentTreeSha=` + currentTreeSha)
	if currentTree == "" || currentTreeSha == "" {
		red("Tree empty !")
		err := os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
		exitIfError(err, "main::os.WriteFile")
		os.Exit(5)
	}

	ciHistoryPath := getProjectPath() + "ci_ok_history"

	// 3. Check if the SHA-1 is present in the history file
	historyFileExist := fileExists(ciHistoryPath)
	var history []string
	if historyFileExist {
		historyByte, err := os.ReadFile(ciHistoryPath)
		exitIfError(err, "os.ReadFile(ciHistoryPath)")
		history = strings.Split(string(historyByte), "\n")
	} else {
		history = []string{}
	}
	if isVerbose() {
		verbose("history=\n" + strings.Join(history, "\n"))
	}
	for _, line := range history {
		if strings.HasPrefix(line, currentTreeSha) {
			verbose("foundLine=" + line)
			// 4. If found, write true in "ci-skip", download and extract the artifact of the found job and exit with code 0
			err := os.WriteFile(getCiSkipPath(), []byte("true"), 0644)
			exitIfError(err, "main::os.WriteFile")
			jobId := strings.Split(line, ":")[1]
			green("✅ tree found in job " + jobId)
			err = extractArtifacts(jobId)
			if err != nil {
				verbose("ERROR extractArtifacts(jobId)")
			}
			os.Exit(0)
		}
	}
	// 5. If not found, write false in "ci-skip", append the SHA-1:CI_JOB_ID to "ci_ok_history" and exit 2
	_ = os.WriteFile(getCiSkipPath(), []byte("false"), 0644)
	yellow("❌ tree not found in history")
	if len(history) > HISTORY_MAX {
		history = history[0:HISTORY_MAX]
	}
	newHistoContent := currentTreeSha + ":" + os.Getenv("CI_JOB_ID") + "\n" + strings.Join(history, "\n")
	err = os.WriteFile(ciHistoryPath, []byte(newHistoContent), 0644)
	exitIfError(err, "prepend to ciHistoryPath")
	os.Exit(2)
}
