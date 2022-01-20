package main

import (
	"github.com/go-git/go-git/v5"
	"github.com/go-git/go-git/v5/plumbing"
	"os"
	"strings"
	"testing"
)

func Test_fileExists(t *testing.T) {
	t.Run("fileExists", func(t *testing.T) {
		if fileExists("skipIfTreeOkInPast_test.go") == false {
			t.Errorf("fileExists() true")
		}
		if fileExists("missing") == true {
			t.Errorf("fileExists() false")
		}
	})
}

func Test_getProjectPath(t *testing.T) {
	t.Run("getProjectPath", func(t *testing.T) {
		os.Setenv("CI_BUILDS_DIR", "/builds")
		os.Setenv("CI_PROJECT_DIR", "/builds/test/prj")
		expected := "/builds/test/prj"
		if path := getProjectPath(); path != expected {
			t.Errorf("getProjectPath() = %v, want %v", path, expected)
		}
		os.Setenv("CI_BUILDS_DIR", "/sub/path/builds")
		os.Setenv("CI_PROJECT_DIR", "/builds/test/prj")
		expected = "/sub/path/builds/test/prj"
		if path := getProjectPath(); path != expected {
			t.Errorf("getProjectPath() = %v, want %v", path, expected)
		}

	})
}

func Test_getSha(t *testing.T) {
	t.Run("getSha", func(t *testing.T) {
		str := `c38fa4e005685a861be5fdbe8fcbb03f84a216b0 .gitignore
130990b0713d95f3d30ee72f1bb86c9a3f53f914 .gitlab-ci.yml
`
		expected := "Tiq5DqyLgY6jNJrUe9I3TYwG3PM="
		if sha := getSha(str); sha != expected {
			t.Errorf("getSha() = %v, want %v", sha, expected)
		}
	})
}

func Test_getTreeOfPaths(t *testing.T) {
	t.Run("Test_getTreeOfPaths", func(t *testing.T) {
		repository, err := git.PlainOpen("../")
		paths := strings.Split(".gitignore .gitlab-ci.yml", " ")

		tree, err := getTreeOfPaths(repository, plumbing.NewHash("77f761c4eac13a0b2fe0f20a57c281f07858c0f1"), paths)
		if err != nil {
			t.Errorf("getTreeOfPaths() error = %v", err)
			return
		}
		expected := `c38fa4e005685a861be5fdbe8fcbb03f84a216b0 .gitignore
130990b0713d95f3d30ee72f1bb86c9a3f53f914 .gitlab-ci.yml
`
		if tree != expected {
			t.Errorf("getTreeOfPaths() tree = %v", tree)
			return
		}
	})
}
