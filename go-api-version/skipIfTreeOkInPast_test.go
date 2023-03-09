package main

import (
	"bytes"
	"encoding/json"
	"errors"
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	untar "github.com/jersou/gitlab-skip-if-tree-ok-in-past/test"
	"io/ioutil"
	"log"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
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
	if os.Getenv("TEST_EXIT_FORK") == "true" {
		exitError("test")
	} else {
		cmd := exec.Command(os.Args[0], "-test.run=Test_exitError")
		cmd.Env = append(os.Environ(), "TEST_EXIT_FORK=true")
		err := cmd.Run()
		if e, ok := err.(*exec.ExitError); !ok || e.ExitCode() != 1 {
			t.Fatalf("process ran with err %v, want exit status 1", e.ExitCode())
		}
	}
}

func Test_exitIfError_no_error(t *testing.T) {
	if os.Getenv("TEST_EXIT_FORK") == "true" {
		exitIfError(nil, "test")
	} else {
		cmd := exec.Command(os.Args[0], "-test.run=Test_exitIfError")
		cmd.Env = append(os.Environ(), "TEST_EXIT_FORK=true")
		err := cmd.Run()
		if e, ok := err.(*exec.ExitError); !ok {
			t.Fatalf("process ran with err %v, want exit status 0", e.ExitCode())
		}
	}
}

func Test_exitIfError_error(t *testing.T) {
	if os.Getenv("TEST_EXIT_FORK") == "true" {
		exitIfError(errors.New("err"), "test")
	} else {
		cmd := exec.Command(os.Args[0], "-test.run=Test_exitIfError")
		cmd.Env = append(os.Environ(), "TEST_EXIT_FORK=true")
		err := cmd.Run()
		if e, ok := err.(*exec.ExitError); !ok || e.ExitCode() != 1 {
			t.Fatalf("process ran with err %v, want exit status 1", e.ExitCode())
		}
	}
}

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
		println(r)
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
