#!/usr/bin/env bash
usage() {
  echo "Usage $0 [-h | -c | -b <branch>]"
  echo
  echo "Checks for changes in the source code of the Budgeteur-rs git repo."
  echo "Exits with 0 if no change is detected, 1 if a change is detected, "
  echo "and 2 if an error occurred."
  echo
  echo "Options:"
  echo " -h           Display this help message"
  echo " -c           Diff HEAD with the previous commit on the current branch (HEAD^)"
  echo " -b <branch>  Diff HEAD on the current branch with HEAD on the specified branch"
}

while getopts "hcb:" flag; do
  case $flag in
    h)
      usage
      exit 0
      ;;
    c)
      diff_mode=previous_commit
      ;;
    b)
      diff_mode=branch
      diff_branch=$OPTARG

      if [[ -z $diff_branch ]]; then
        echo "Branch name cannot be empty" >&2
        exit 2
      fi
      ;;
    *)
      # echo to add blank line between error message and help message
      echo
      usage
      exit 2
      ;;
  esac
done

if [[ -z $diff_mode ]]; then
  echo "Diff mode must be specified" >&2
  echo
  usage
  exit 1
fi

current_branch=$(git branch --show-current)

case $diff_mode in
  previous_commit)
    echo "Diffing HEAD with HEAD^ on branch $current_branch"
    CHANGED_FILES=$(git diff --name-only HEAD^ HEAD)

    if [[ "$?" -ne 0 ]]; then
      exit 2
    fi
    ;;
  branch)
    echo "Diffing branch $current_branch against branch $diff_branch"
    CHANGED_FILES=$(git diff --name-only $diff_branch)

    if [[ "$?" -ne 0 ]]; then
      exit 2
    fi
    ;;
  *)
    echo "Oops! The programmer made a mistake!" >&2
    exit 2
    ;;
esac

SOURCE_FILES="src/ templates/ static/ Cargo.toml Cargo.lock Dockerfile"
CHANGE_FOUND=false

echo "Looking for files equal to or starting with: $SOURCE_FILES"
echo "Found changed files: $CHANGED_FILES"

for line in $CHANGED_FILES; do
  for file_prefix in $SOURCE_FILES; do
    if [[ $line == $file_prefix* ]]; then
      echo "$line is a source file (matched pattern \"$file_prefix*\")"
      CHANGE_FOUND=true
      break 2
    fi
  done

  if [[ $CHANGE_FOUND == true ]]; then
    break
  fi
done

if [[ $CHANGE_FOUND == true ]]; then
  echo Source has changed
  exit 1
else
  echo Source has not changed
fi

exit 0
