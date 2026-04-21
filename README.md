## 카카오톡 다중 로그인 패치 도구

카카오톡은 공식적으로 갤럭시 태블릿에서만 다중 기기 로그인을 지원합니다. 하지만 약간의 패치를 통하면 모든 안드로이드 기기에서도 다중 로그인 기능을 사용할 수 있습니다.

이 패치 도구는 [LSPatch](https://github.com/JingMatrix/LSPatch)를 사용하여 Google Play에서 다운로드한 원본 카카오톡 APK에 [TabletSpoof](https://github.com/miner7222/TabletSpoof) 패치를 적용하는 과정을 자동화합니다.

## 사용 방법

### 패치 도구 다운로드

[여기](https://github.com/ny0510/kakaotalk-tablet-patcher/releases)에서 OS에 알맞는 파일을 다운로드하세요.
Windows, macOS, Linux를 지원합니다.

### 패치 적용하기

다운로드한 패치 도구가 있는 폴더에서 터미널을 열고 다음 명령어를 실행하세요.

```bash
# Windows
.\kakaotalk-tablet-patcher-windows.exe run

# Linux
./kakaotalk-tablet-patcher-linux run

# macOS
./kakaotalk-tablet-patcher-macos run
```

`run` 옵션을 사용하면 패치 도구가 자동으로 최신 버전의 LSPatch, TabletSpoof와 카카오톡 APK를 다운로드하여 패치를 진행합니다.

패치가 완료되면 `output` 폴더에 `KakaoTalk-Patched.apks` 파일이 생성됩니다.

### 커스텀 APK로 패치하기

`--apk` 옵션을 사용하면 Google Play에서 자동 다운로드하는 대신 로컬에 존재하는 카카오톡 APK로 패치할 수 있습니다. 구버전 패치, 다른 아키텍처 APK, 다른 언어 APK 등이 필요한 경우에 사용하세요.

```bash
# 단일 APK 파일로 패치
./kakaotalk-tablet-patcher-macos run --apk ./kakaotalk.apk

# 분활된 APK가 있는 경우
./kakaotalk-tablet-patcher-macos run --apk ./base.apk --splits-dir ./splits/

# patch 서브커맨드에도 동일하게 사용 가능
./kakaotalk-tablet-patcher-macos patch --apk ./kakaotalk.apk
```

> `--splits-dir`은 `--apk`와 함께만 사용할 수 있습니다. 지정한 디렉토리 내의 `.apk` 파일들이 자동으로 포함됩니다.

### 패치된 APKs 설치하기

APKs 파일을 설치하기 위해서는 [Android Split APKs Installer](https://github.com/aefyr/SAI)와 같은 도구가 필요합니다.

APKs 파일을 설치할 기기에 복사한 뒤, SAI 앱을 열고 `KakaoTalk-Patched.apks` 파일을 선택하여 설치를 진행하세요.

또는 아래 ADB 명령어를 사용하여 설치할 수도 있습니다.

```bash
adb install-multiple -r output/*.apk
```
