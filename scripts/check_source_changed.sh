#!/bin/bash
SOURCE_FILES="src/ templates/ static/ Cargo.toml Cargo.lock Dockerfile"
CHANGED_FILES=$(git diff --name-only HEAD^ HEAD)
CHANGE_FOUND=false

echo "Looking for files equal to or starting with: $SOURCE_FILES"
echo "Found changed files: $CHANGED_FILES"

for line in $CHANGED_FILES; do
  for file_prefix in $SOURCE_FILES; do
    if [[ $line == $file_prefix* ]]; then
      echo "$file_prefix is a source file (matched pattern \"$line*\")"
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
