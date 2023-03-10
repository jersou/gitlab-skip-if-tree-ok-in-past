package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	untar "github.com/jersou/gitlab-skip-if-tree-ok-in-past/test"
	"io/ioutil"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func Test_getProjectPath(t *testing.T) {
	defer os.Unsetenv("CI_BUILDS_DIR")
	defer os.Unsetenv("CI_PROJECT_DIR")

	os.Setenv("CI_BUILDS_DIR", "/builds")
	os.Setenv("CI_PROJECT_DIR", "/builds/test/prj")
	expected := "/builds/test/prj/"
	if path := getProjectPath(); path != expected {
		t.Errorf("getProjectPath() = %v, want %v", path, expected)
	}
	os.Setenv("CI_BUILDS_DIR", "/sub/path/builds")
	os.Setenv("CI_PROJECT_DIR", "/builds/test/prj")
	expected = "/sub/path/builds/test/prj/"
	if path := getProjectPath(); path != expected {
		t.Errorf("getProjectPath() = %v, want %v", path, expected)
	}
}

func Test_help(t *testing.T) {
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	printHelp()
	got := buf.String()
	if len(got) < 1000 {
		t.Errorf("printHelp error : len = %v", len(got))
	}
}

func Test_verbose_enabled(t *testing.T) {
	defer os.Unsetenv("SKIP_CI_VERBOSE")
	os.Setenv("SKIP_CI_VERBOSE", "true")
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	verbose("test")
	expected := "test"
	got := buf.String()
	if strings.HasSuffix(buf.String(), expected) {
		t.Errorf("got <%v>, want <%v>", got, expected)
	}
}

func Test_verbose_disabled(t *testing.T) {
	os.Unsetenv("SKIP_CI_VERBOSE")
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	verbose("test")
	expected := ""
	got := buf.String()
	if buf.String() != expected {
		t.Errorf("got <%v>, want <%v>", got, expected)
	}
}

func Test_red(t *testing.T) {
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	red("test")
	expected := "\033[1;41;30m   test   \033[0m"
	got := buf.String()
	if strings.HasSuffix(buf.String(), expected) {
		t.Errorf("got %v, want %v", got, expected)
	}
}

func Test_yellow(t *testing.T) {
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	yellow("test")
	expected := "\033[1;43;30m   test   \033[0m"
	got := buf.String()
	if strings.HasSuffix(buf.String(), expected) {
		t.Errorf("got %v, want %v", got, expected)
	}
}

func Test_green(t *testing.T) {
	var buf bytes.Buffer
	log.SetOutput(&buf)
	defer log.SetOutput(os.Stderr)
	green("test")
	expected := "\033[1;42;30m   test   \033[0m"
	got := buf.String()
	if strings.HasSuffix(buf.String(), expected) {
		t.Errorf("got %v, want %v", got, expected)
	}
}

func Test_exitError(t *testing.T) {
	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")
	exitCode := exitError("test")
	if exitCode != 1 {
		t.Fatalf("process ran with err %v, want exit status 1", exitCode)
	}
}

func Test_exitIfError_no_error(t *testing.T) {
	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")
	exitCode := exitIfError(nil, "test")
	if exitCode != -1 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
}

func Test_exitIfError_error(t *testing.T) {
	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")
	exitCode := exitIfError(errors.New("err"), "test")
	if exitCode != 1 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
}

func Test_exitNotFound(t *testing.T) {
	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")

	tmpDir, err := ioutil.TempDir("", "Test_exitNotFound")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")
	defer os.Unsetenv("CI_PROJECT_ID")
	os.Setenv("CI_PROJECT_ID", "CI_PROJECT_ID")
	defer os.Unsetenv("CI_JOB_ID")
	os.Setenv("CI_JOB_ID", "CI_JOB_ID")

	defer os.Unsetenv("CI_BUILDS_DIR")
	os.Setenv("CI_BUILDS_DIR", tmpDir)
	defer os.Unsetenv("CI_PROJECT_DIR")
	os.Setenv("CI_PROJECT_DIR", tmpDir)

	exitCode := exitNotFound()
	if exitCode != 4 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
}

func Test_initCheck(t *testing.T) {
	defer os.Unsetenv("GO_TEST")
	os.Setenv("GO_TEST", "true")
	if exitCode := initCheck([]string{"test", "test"}); exitCode != 1 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
	if exitCode := initCheck([]string{}); exitCode != 1 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
	defer os.Unsetenv("SKIP_IF_TREE_OK_IN_PAST")
	os.Setenv("SKIP_IF_TREE_OK_IN_PAST", "file1 file2")
	if exitCode := initCheck([]string{}); exitCode != 2 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}

	tmpDir, err := ioutil.TempDir("", "Test_initCheck")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	defer os.Unsetenv("CI_BUILDS_DIR")
	os.Setenv("CI_BUILDS_DIR", tmpDir)
	defer os.Unsetenv("CI_PROJECT_DIR")
	os.Setenv("CI_PROJECT_DIR", tmpDir)
	defer os.Unsetenv("CI_PROJECT_ID")
	os.Setenv("CI_PROJECT_ID", "CI_PROJECT_ID")
	defer os.Unsetenv("CI_JOB_ID")
	os.Setenv("CI_JOB_ID", "CI_JOB_ID")

	f, err := os.Create(tmpDir + "/ci-skip-CI_PROJECT_ID-CI_JOB_ID")

	defer os.Unsetenv("API_READ_TOKEN")
	os.Setenv("API_READ_TOKEN", "API_READ_TOKEN")

	if exitCode := initCheck([]string{}); exitCode != 3 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}

	f.WriteString("true")
	if exitCode := initCheck([]string{}); exitCode != 0 {
		t.Fatalf("exitIfError exit with code %v", exitCode)
	}
}

//func Test_extractArtifacts(t *testing.T){
//	// TODO
//
//	tmpDir, err := ioutil.TempDir("", "Test_getTreeOfPaths")
//	if err != nil {
//		log.Fatal(err)
//	}
//	defer os.RemoveAll(tmpDir)
//
//	os.Chdir(tmpDir)
//
//}

func Test_getTreeOfPaths(t *testing.T) {
	tmpDir, err := ioutil.TempDir("", "Test_getTreeOfPaths")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	tarFile, err := os.Open("test/repo.tar.gz")
	err = untar.Untar(tarFile, tmpDir)
	if err != nil {
		return
	}

	repository, err := git.PlainOpen(tmpDir)

	currentTree, err := getTreeOfPaths(repository,
		plumbing.NewHash("bd2774d24726e2b9ec20029f1e2058a1d360abcd"),
		[]string{"folder1", "a", "b"})

	expected := "621b48b2576800166cb40216c71f8e63ec2b8990 folder1\nf70f10e4db19068f79bc43844b49f3eece45c4e8 a\n223b7836fb19fdf64ba2d3cd6173c6a283141f78 b\n"

	if currentTree != expected {
		t.Errorf("got %v, want %v", currentTree, expected)
	}

	currentTree, err = getTreeOfPaths(repository,
		plumbing.NewHash("0000000000000000000000000000000000000000"),
		[]string{"folder1", "a", "b"})

	expected = ""
	if currentTree != expected {
		t.Errorf("got %v, want %v", currentTree, expected)
	}

	currentTree, err = getTreeOfPaths(repository,
		plumbing.NewHash("bd2774d24726e2b9ec20029f1e2058a1d360abcd"),
		[]string{"folder1", "aaa", "b"})

	expected = ""
	if currentTree != expected {
		t.Errorf("got %v, want %v", currentTree, expected)
	}
}

func Test_getProjectJobs(t *testing.T) {
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte(`[{"Id":123}]`))
	}))

	defer os.Unsetenv("CI_API_V4_URL")
	defer os.Unsetenv("CI_PROJECT_ID")
	defer os.Unsetenv("API_READ_TOKEN")
	os.Setenv("CI_API_V4_URL", server.URL)
	os.Setenv("CI_PROJECT_ID", "1234")
	os.Setenv("API_READ_TOKEN", "AAAAAAAAAA")

	res := getProjectJobs(1)

	expected := `[{"Id":123,"Name":"","Ref":"","Web_url":"","Artifacts_expire_at":"","Commit":{"Id":""}}]`
	got, _ := json.Marshal(res)
	if string(got) != expected {
		t.Errorf("got %v, want %v", string(got), expected)
	}
}

func Test_downloadFile(t *testing.T) {
	tmpFile, err := ioutil.TempFile("", "Test_downloadFile")
	defer os.Remove(tmpFile.Name())
	if err != nil {
		log.Fatal(err)
	}
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Write([]byte("Test_downloadFile"))
	}))
	downloadFile(tmpFile.Name(), server.URL)
	content, _ := os.ReadFile(tmpFile.Name())
	expected := "Test_downloadFile"

	if string(content) != expected {
		t.Errorf("got %v, want %v", string(content), expected)
	}
}

func Test_extractArchive(t *testing.T) {
	tmpDir, err := ioutil.TempDir("", "Test_extractArchive")
	if err != nil {
		log.Fatal(err)
	}
	defer os.RemoveAll(tmpDir)

	extractArchive("test/artifact.zip", tmpDir)

	paths := ""

	filepath.Walk(tmpDir, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			fmt.Println(err)
			return err
		}
		paths += path[len(tmpDir):] + "\n"
		return nil
	})
	expected := "\n/artifact\n/artifact/a\n/artifact/b\n/artifact/c\n/artifact/folder1\n/artifact/folder1/d\n/artifact/folder1/e\n/artifact/folder1/f\n/artifact/folder2\n/artifact/folder2/g\n/artifact/folder2/h\n/artifact/folder2/i\n"

	if paths != expected {
		t.Errorf("got <%v>, want <%v>", paths, expected)
	}
}
