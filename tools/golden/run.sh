#!/usr/bin/env bash
# Regenerates src-tauri/tests/data from the Android source. Uses the Kotlin compiler already in the Gradle cache.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
kt="${EMRAN_ANDROID:-/mnt/windows/Android Projects/Emran/emran}/app/src/main/java/io/github/sammynashed/emran/data/PrayerTimeCalculator.kt"
java="${JAVA:-$HOME/jdk21/bin/java}"
G="$HOME/.gradle/caches/modules-2/files-2.1"
pick() { find "$G/$1" -name "$2" ! -name '*sources*' 2>/dev/null | sort | tail -1; }
stdlib=$(pick org.jetbrains.kotlin/kotlin-stdlib/2.2.10 'kotlin-stdlib-2.2.10.jar')
cp="$(pick org.jetbrains.kotlin/kotlin-compiler-embeddable/2.2.10 '*.jar'):$stdlib:$(pick org.jetbrains.kotlin/kotlin-script-runtime/2.2.10 '*.jar'):$(pick org.jetbrains.kotlin/kotlin-reflect/2.2.10 '*.jar'):$(pick org.jetbrains.kotlin/kotlin-daemon-embeddable/2.2.10 '*.jar'):$(pick org.jetbrains.intellij.deps/trove4j '*.jar'):$(pick org.jetbrains.intellij.deps.kotlinx/kotlinx-coroutines-core-jvm '*.jar'):$(pick org.jetbrains/annotations '*.jar')"
out="$(mktemp -d)"; trap 'rm -rf "$out"' EXIT
"$java" -cp "$cp" org.jetbrains.kotlin.cli.jvm.K2JVMCompiler -nowarn -no-stdlib -no-reflect -cp "$stdlib" -d "$out" "$kt" "$here/GoldenDump.kt"
"$java" -cp "$out:$stdlib" GoldenDumpKt "$here/../../core"
