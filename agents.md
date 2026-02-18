do we need to add a way for the [CI](./.github/workflows/CI.yml) to incorporate pdfium?

```log
2026-02-18T12:48:59.0799550Z Current runner version: '2.331.0'
2026-02-18T12:48:59.0813430Z ##[group]Runner Image Provisioner
2026-02-18T12:48:59.0813900Z Hosted Compute Agent
2026-02-18T12:48:59.0814230Z Version: 20260213.493
2026-02-18T12:48:59.0814600Z Commit: 5c115507f6dd24b8de37d8bbe0bb4509d0cc0fa3
2026-02-18T12:48:59.0815020Z Build Date: 2026-02-13T00:28:41Z
2026-02-18T12:48:59.0815420Z Worker ID: {d3b947cc-fecb-45a1-9bac-03fff9231966}
2026-02-18T12:48:59.0815840Z Azure Region: westus
2026-02-18T12:48:59.0816170Z ##[endgroup]
2026-02-18T12:48:59.0816960Z ##[group]Operating System
2026-02-18T12:48:59.0817290Z macOS
2026-02-18T12:48:59.0817560Z 15.7.3
2026-02-18T12:48:59.0817820Z 24G419
2026-02-18T12:48:59.0818100Z ##[endgroup]
2026-02-18T12:48:59.0818390Z ##[group]Runner Image
2026-02-18T12:48:59.0818700Z Image: macos-15-arm64
2026-02-18T12:48:59.0819010Z Version: 20260209.0147.2
2026-02-18T12:48:59.0819760Z Included Software: https://github.com/actions/runner-images/blob/macos-15-arm64/20260209.0147/images/macos/macos-15-arm64-Readme.md
2026-02-18T12:48:59.0820740Z Image Release: https://github.com/actions/runner-images/releases/tag/macos-15-arm64%2F20260209.0147
2026-02-18T12:48:59.0821330Z ##[endgroup]
2026-02-18T12:48:59.0822020Z ##[group]GITHUB_TOKEN Permissions
2026-02-18T12:48:59.0822930Z Contents: read
2026-02-18T12:48:59.0823260Z Metadata: read
2026-02-18T12:48:59.0823560Z Packages: read
2026-02-18T12:48:59.0823880Z ##[endgroup]
2026-02-18T12:48:59.0825320Z Secret source: Actions
2026-02-18T12:48:59.0825710Z Prepare workflow directory
2026-02-18T12:48:59.1075850Z Prepare all required actions
2026-02-18T12:48:59.1103440Z Getting action download info
2026-02-18T12:48:59.5119910Z Download action repository 'actions/checkout@v6' (SHA:de0fac2e4500dabe0009e67214ff5f5447ce83dd)
2026-02-18T12:48:59.7703140Z Download action repository 'actions/setup-node@v6' (SHA:6044e13b5dc448c55e2357c09f80417699197238)
2026-02-18T12:48:59.9159080Z Download action repository 'dtolnay/rust-toolchain@stable' (SHA:631a55b12751854ce901bb631d5902ceb48146f7)
2026-02-18T12:49:00.4216710Z Download action repository 'actions/cache@v5' (SHA:cdf6c1fa76f9f475f3d7449005a359c84ca0f306)
2026-02-18T12:49:01.2461000Z Download action repository 'actions/upload-artifact@v6' (SHA:b7c566a772e6b6bfb58ed0dc250532a479d7789f)
2026-02-18T12:49:01.4837970Z Complete job name: OCR - aarch64-apple-darwin
2026-02-18T12:49:01.5358150Z ##[group]Run actions/checkout@v6
2026-02-18T12:49:01.5358980Z with:
2026-02-18T12:49:01.5359540Z   repository: DopamineDriven/pdfdown
2026-02-18T12:49:01.5360500Z   token: ***
2026-02-18T12:49:01.5360970Z   ssh-strict: true
2026-02-18T12:49:01.5361470Z   ssh-user: git
2026-02-18T12:49:01.5362000Z   persist-credentials: true
2026-02-18T12:49:01.5362560Z   clean: true
2026-02-18T12:49:01.5363100Z   sparse-checkout-cone-mode: true
2026-02-18T12:49:01.5363720Z   fetch-depth: 1
2026-02-18T12:49:01.5364220Z   fetch-tags: false
2026-02-18T12:49:01.5364730Z   show-progress: true
2026-02-18T12:49:01.5365270Z   lfs: false
2026-02-18T12:49:01.5365760Z   submodules: false
2026-02-18T12:49:01.5366320Z   set-safe-directory: true
2026-02-18T12:49:01.5367090Z env:
2026-02-18T12:49:01.5367540Z   DEBUG: napi:*
2026-02-18T12:49:01.5368040Z   APP_NAME: pdfdown
2026-02-18T12:49:01.5368560Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:01.5369150Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:01.5369730Z ##[endgroup]
2026-02-18T12:49:01.9015520Z Syncing repository: DopamineDriven/pdfdown
2026-02-18T12:49:01.9017920Z ##[group]Getting Git version info
2026-02-18T12:49:01.9018760Z Working directory is '/Users/runner/work/pdfdown/pdfdown'
2026-02-18T12:49:01.9019910Z [command]/opt/homebrew/bin/git version
2026-02-18T12:49:01.9212540Z git version 2.52.0
2026-02-18T12:49:01.9227690Z ##[endgroup]
2026-02-18T12:49:01.9234020Z Copying '/Users/runner/.gitconfig' to '/Users/runner/work/_temp/cc0a96aa-ec78-4dad-aa3a-10406233effe/.gitconfig'
2026-02-18T12:49:01.9239910Z Temporarily overriding HOME='/Users/runner/work/_temp/cc0a96aa-ec78-4dad-aa3a-10406233effe' before making global git config changes
2026-02-18T12:49:01.9242430Z Adding repository directory to the temporary git global config as a safe directory
2026-02-18T12:49:01.9245270Z [command]/opt/homebrew/bin/git config --global --add safe.directory /Users/runner/work/pdfdown/pdfdown
2026-02-18T12:49:01.9344370Z Deleting the contents of '/Users/runner/work/pdfdown/pdfdown'
2026-02-18T12:49:01.9346720Z ##[group]Initializing the repository
2026-02-18T12:49:01.9350860Z [command]/opt/homebrew/bin/git init /Users/runner/work/pdfdown/pdfdown
2026-02-18T12:49:01.9553420Z hint: Using 'master' as the name for the initial branch. This default branch name
2026-02-18T12:49:01.9556490Z hint: will change to "main" in Git 3.0. To configure the initial branch name
2026-02-18T12:49:01.9558330Z hint: to use in all of your new repositories, which will suppress this warning,
2026-02-18T12:49:01.9570240Z hint: call:
2026-02-18T12:49:01.9570920Z hint:
2026-02-18T12:49:01.9571610Z hint: 	git config --global init.defaultBranch <name>
2026-02-18T12:49:01.9572490Z hint:
2026-02-18T12:49:01.9574120Z hint: Names commonly chosen instead of 'master' are 'main', 'trunk' and
2026-02-18T12:49:01.9575370Z hint: 'development'. The just-created branch can be renamed via this command:
2026-02-18T12:49:01.9576430Z hint:
2026-02-18T12:49:01.9577040Z hint: 	git branch -m <name>
2026-02-18T12:49:01.9577690Z hint:
2026-02-18T12:49:01.9578540Z hint: Disable this message with "git config set advice.defaultBranchName false"
2026-02-18T12:49:01.9579970Z Initialized empty Git repository in /Users/runner/work/pdfdown/pdfdown/.git/
2026-02-18T12:49:01.9582280Z [command]/opt/homebrew/bin/git remote add origin https://github.com/DopamineDriven/pdfdown
2026-02-18T12:49:01.9750900Z ##[endgroup]
2026-02-18T12:49:01.9751980Z ##[group]Disabling automatic garbage collection
2026-02-18T12:49:01.9753130Z [command]/opt/homebrew/bin/git config --local gc.auto 0
2026-02-18T12:49:02.0111530Z ##[endgroup]
2026-02-18T12:49:02.0214220Z ##[group]Setting up auth
2026-02-18T12:49:02.0322600Z Removing SSH command configuration
2026-02-18T12:49:02.0427140Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp core\.sshCommand
2026-02-18T12:49:02.0468890Z [command]/opt/homebrew/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'core\.sshCommand' && git config --local --unset-all 'core.sshCommand' || :"
2026-02-18T12:49:02.1172530Z Removing HTTP extra header
2026-02-18T12:49:02.1174050Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp http\.https\:\/\/github\.com\/\.extraheader
2026-02-18T12:49:02.1222190Z [command]/opt/homebrew/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'http\.https\:\/\/github\.com\/\.extraheader' && git config --local --unset-all 'http.https://github.com/.extraheader' || :"
2026-02-18T12:49:02.2079650Z Removing includeIf entries pointing to credentials config files
2026-02-18T12:49:02.2114160Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp ^includeIf\.gitdir:
2026-02-18T12:49:02.2117220Z [command]/opt/homebrew/bin/git submodule foreach --recursive git config --local --show-origin --name-only --get-regexp remote.origin.url
2026-02-18T12:49:02.2780420Z [command]/opt/homebrew/bin/git config --file /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config http.https://github.com/.extraheader AUTHORIZATION: basic ***
2026-02-18T12:49:02.2843820Z [command]/opt/homebrew/bin/git config --local includeIf.gitdir:/Users/runner/work/pdfdown/pdfdown/.git.path /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:49:02.2903040Z [command]/opt/homebrew/bin/git config --local includeIf.gitdir:/Users/runner/work/pdfdown/pdfdown/.git/worktrees/*.path /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:49:02.2966990Z [command]/opt/homebrew/bin/git config --local includeIf.gitdir:/github/workspace/.git.path /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:49:02.3024900Z [command]/opt/homebrew/bin/git config --local includeIf.gitdir:/github/workspace/.git/worktrees/*.path /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:49:02.3081360Z ##[endgroup]
2026-02-18T12:49:02.3082590Z ##[group]Fetching the repository
2026-02-18T12:49:02.3085990Z [command]/opt/homebrew/bin/git -c protocol.version=2 fetch --no-tags --prune --no-recurse-submodules --depth=1 origin +d8b32e5b0c7126f87dec868e4b345f5fdbc3774c:refs/remotes/origin/main
2026-02-18T12:49:02.9605010Z From https://github.com/DopamineDriven/pdfdown
2026-02-18T12:49:02.9621620Z  * [new ref]         d8b32e5b0c7126f87dec868e4b345f5fdbc3774c -> origin/main
2026-02-18T12:49:02.9737030Z [command]/opt/homebrew/bin/git branch --list --remote origin/main
2026-02-18T12:49:02.9793500Z   origin/main
2026-02-18T12:49:02.9840530Z [command]/opt/homebrew/bin/git rev-parse refs/remotes/origin/main
2026-02-18T12:49:02.9864540Z d8b32e5b0c7126f87dec868e4b345f5fdbc3774c
2026-02-18T12:49:02.9944870Z ##[endgroup]
2026-02-18T12:49:03.0034680Z ##[group]Determining the checkout info
2026-02-18T12:49:03.0075070Z ##[endgroup]
2026-02-18T12:49:03.0075520Z [command]/opt/homebrew/bin/git sparse-checkout disable
2026-02-18T12:49:03.0076990Z [command]/opt/homebrew/bin/git config --local --unset-all extensions.worktreeConfig
2026-02-18T12:49:03.0121150Z ##[group]Checking out the ref
2026-02-18T12:49:03.0179460Z [command]/opt/homebrew/bin/git checkout --progress --force -B main refs/remotes/origin/main
2026-02-18T12:49:03.0341330Z Switched to a new branch 'main'
2026-02-18T12:49:03.0362710Z branch 'main' set up to track 'origin/main'.
2026-02-18T12:49:03.0424790Z ##[endgroup]
2026-02-18T12:49:03.0462540Z [command]/opt/homebrew/bin/git log -1 --format=%H
2026-02-18T12:49:03.0542430Z d8b32e5b0c7126f87dec868e4b345f5fdbc3774c
2026-02-18T12:49:03.0740430Z ##[group]Run actions/setup-node@v6
2026-02-18T12:49:03.0741040Z with:
2026-02-18T12:49:03.0741190Z   node-version: 24
2026-02-18T12:49:03.0741370Z   cache: yarn
2026-02-18T12:49:03.0741520Z   check-latest: false
2026-02-18T12:49:03.0741770Z   token: ***
2026-02-18T12:49:03.0741920Z   package-manager-cache: true
2026-02-18T12:49:03.0742110Z env:
2026-02-18T12:49:03.0742230Z   DEBUG: napi:*
2026-02-18T12:49:03.0742370Z   APP_NAME: pdfdown
2026-02-18T12:49:03.0742540Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:03.0742790Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:03.0742940Z ##[endgroup]
2026-02-18T12:49:03.3326700Z Found in cache @ /Users/runner/hostedtoolcache/node/24.13.0/arm64
2026-02-18T12:49:03.3429010Z ##[group]Environment details
2026-02-18T12:49:04.3199520Z node: v24.13.0
2026-02-18T12:49:04.3244270Z npm: 11.6.2
2026-02-18T12:49:04.3277870Z yarn: 4.12.0
2026-02-18T12:49:04.3346120Z ##[endgroup]
2026-02-18T12:49:04.3379590Z [command]/Users/runner/.yarn/bin/yarn --version
2026-02-18T12:49:04.7042150Z 4.12.0
2026-02-18T12:49:04.7244790Z [command]/Users/runner/.yarn/bin/yarn config get cacheFolder
2026-02-18T12:49:05.0272970Z /Users/runner/.yarn/berry/cache
2026-02-18T12:49:05.1614580Z [command]/Users/runner/.yarn/bin/yarn config get enableGlobalCache
2026-02-18T12:49:05.3652850Z [33mtrue[39m
2026-02-18T12:49:05.5929930Z Cache hit for: node-cache-macOS-arm64-yarn-c53c71cbec123083256fad35bd794fd7c2e247f2bc84f4bd923a8c5547d7cfc5
2026-02-18T12:49:06.4881810Z Received 35340193 of 35340193 (100.0%), 45.3 MBs/sec
2026-02-18T12:49:06.4885940Z Cache Size: ~34 MB (35340193 B)
2026-02-18T12:49:06.4915470Z [command]/opt/homebrew/bin/gtar -xf /Users/runner/work/_temp/ad9efb5a-f3d6-4da4-987f-3945c356a74c/cache.tzst -P -C /Users/runner/work/pdfdown/pdfdown --delay-directory-restore --use-compress-program unzstd
2026-02-18T12:49:06.9345170Z Cache restored successfully
2026-02-18T12:49:06.9356470Z Cache restored from key: node-cache-macOS-arm64-yarn-c53c71cbec123083256fad35bd794fd7c2e247f2bc84f4bd923a8c5547d7cfc5
2026-02-18T12:49:06.9651310Z ##[group]Run dtolnay/rust-toolchain@stable
2026-02-18T12:49:06.9651620Z with:
2026-02-18T12:49:06.9680350Z   toolchain: stable
2026-02-18T12:49:06.9680910Z   targets: aarch64-apple-darwin
2026-02-18T12:49:06.9681670Z env:
2026-02-18T12:49:06.9682160Z   DEBUG: napi:*
2026-02-18T12:49:06.9682720Z   APP_NAME: pdfdown
2026-02-18T12:49:06.9683280Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:06.9683830Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:06.9684370Z ##[endgroup]
2026-02-18T12:49:06.9777590Z ##[group]Run : parse toolchain version
2026-02-18T12:49:06.9777990Z [36;1m: parse toolchain version[0m
2026-02-18T12:49:06.9778270Z [36;1mif [[ -z $toolchain ]]; then[0m
2026-02-18T12:49:06.9778770Z [36;1m  # GitHub does not enforce `required: true` inputs itself. https://github.com/actions/runner/issues/1070[0m
2026-02-18T12:49:06.9779330Z [36;1m  echo "'toolchain' is a required input" >&2[0m
2026-02-18T12:49:06.9779620Z [36;1m  exit 1[0m
2026-02-18T12:49:06.9780060Z [36;1melif [[ $toolchain =~ ^stable' '[0-9]+' '(year|month|week|day)s?' 'ago$ ]]; then[0m
2026-02-18T12:49:06.9780450Z [36;1m  if [[ macOS == macOS ]]; then[0m
2026-02-18T12:49:06.9780870Z [36;1m    echo "toolchain=1.$((($(date -v-$(sed 's/stable \([0-9]*\) \(.\).*/\1\2/' <<< $toolchain) +%s)/60/60/24-16569)/7/6))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:06.9781250Z [36;1m  else[0m
2026-02-18T12:49:06.9781570Z [36;1m    echo "toolchain=1.$((($(date --date "${toolchain#stable }" +%s)/60/60/24-16569)/7/6))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:06.9781940Z [36;1m  fi[0m
2026-02-18T12:49:06.9782160Z [36;1melif [[ $toolchain =~ ^stable' 'minus' '[0-9]+' 'releases?$ ]]; then[0m
2026-02-18T12:49:06.9782560Z [36;1m  echo "toolchain=1.$((($(date +%s)/60/60/24-16569)/7/6-${toolchain//[^0-9]/}))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:06.9782920Z [36;1melif [[ $toolchain =~ ^1\.[0-9]+$ ]]; then[0m
2026-02-18T12:49:06.9783490Z [36;1m  echo "toolchain=1.$((i=${toolchain#1.}, c=($(date +%s)/60/60/24-16569)/7/6, i+9*i*(10*i<=c)+90*i*(100*i<=c)))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:06.9783940Z [36;1melse[0m
2026-02-18T12:49:06.9784130Z [36;1m  echo "toolchain=$toolchain" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:06.9784360Z [36;1mfi[0m
2026-02-18T12:49:06.9831240Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:06.9831610Z env:
2026-02-18T12:49:06.9831840Z   DEBUG: napi:*
2026-02-18T12:49:06.9831970Z   APP_NAME: pdfdown
2026-02-18T12:49:06.9832130Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:06.9832300Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:06.9832450Z   toolchain: stable
2026-02-18T12:49:06.9832580Z ##[endgroup]
2026-02-18T12:49:07.0355390Z ##[group]Run : construct rustup command line
2026-02-18T12:49:07.0355710Z [36;1m: construct rustup command line[0m
2026-02-18T12:49:07.0356070Z [36;1mecho "targets=$(for t in ${targets//,/ }; do echo -n ' --target' $t; done)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:07.0356700Z [36;1mecho "components=$(for c in ${components//,/ }; do echo -n ' --component' $c; done)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:07.0357090Z [36;1mecho "downgrade=" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:07.0389420Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:07.0389700Z env:
2026-02-18T12:49:07.0389850Z   DEBUG: napi:*
2026-02-18T12:49:07.0390020Z   APP_NAME: pdfdown
2026-02-18T12:49:07.0390200Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:07.0390400Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:07.0390580Z   targets: aarch64-apple-darwin
2026-02-18T12:49:07.0390780Z   components: 
2026-02-18T12:49:07.0390940Z ##[endgroup]
2026-02-18T12:49:07.0719160Z ##[group]Run : set $CARGO_HOME
2026-02-18T12:49:07.0719410Z [36;1m: set $CARGO_HOME[0m
2026-02-18T12:49:07.0719680Z [36;1mecho CARGO_HOME=${CARGO_HOME:-"$HOME/.cargo"} >> $GITHUB_ENV[0m
2026-02-18T12:49:07.0752170Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:07.0752460Z env:
2026-02-18T12:49:07.0752600Z   DEBUG: napi:*
2026-02-18T12:49:07.0753280Z   APP_NAME: pdfdown
2026-02-18T12:49:07.0753480Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:07.0753670Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:07.0754170Z ##[endgroup]
2026-02-18T12:49:07.1015060Z ##[group]Run : install rustup if needed
2026-02-18T12:49:07.1015350Z [36;1m: install rustup if needed[0m
2026-02-18T12:49:07.1015640Z [36;1mif ! command -v rustup &>/dev/null; then[0m
2026-02-18T12:49:07.1016270Z [36;1m  curl --proto '=https' --tlsv1.2 --retry 10 --retry-connrefused --location --silent --show-error --fail https://sh.rustup.rs | sh -s -- --default-toolchain none -y[0m
2026-02-18T12:49:07.1016900Z [36;1m  echo "$CARGO_HOME/bin" >> $GITHUB_PATH[0m
2026-02-18T12:49:07.1017160Z [36;1mfi[0m
2026-02-18T12:49:07.1046640Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:07.1046930Z env:
2026-02-18T12:49:07.1047070Z   DEBUG: napi:*
2026-02-18T12:49:07.1047250Z   APP_NAME: pdfdown
2026-02-18T12:49:07.1047430Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:07.1047710Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:07.1047950Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:07.1048140Z ##[endgroup]
2026-02-18T12:49:07.1279410Z ##[group]Run rustup toolchain install stable --target aarch64-apple-darwin --profile minimal --no-self-update
2026-02-18T12:49:07.1280100Z [36;1mrustup toolchain install stable --target aarch64-apple-darwin --profile minimal --no-self-update[0m
2026-02-18T12:49:07.1309180Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:07.1309470Z env:
2026-02-18T12:49:07.1309620Z   DEBUG: napi:*
2026-02-18T12:49:07.1309800Z   APP_NAME: pdfdown
2026-02-18T12:49:07.1309970Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:07.1310170Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:07.1310360Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:07.1310560Z   RUSTUP_PERMIT_COPY_RENAME: 1
2026-02-18T12:49:07.1310760Z ##[endgroup]
2026-02-18T12:49:07.4560310Z info: syncing channel updates for 'stable-aarch64-apple-darwin'
2026-02-18T12:49:07.6187820Z info: latest update on 2026-02-12, rust version 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:49:07.6374930Z info: downloading component 'rust-std'
2026-02-18T12:49:07.9455630Z info: downloading component 'clippy'
2026-02-18T12:49:08.0343810Z info: downloading component 'rustfmt'
2026-02-18T12:49:08.1269790Z info: downloading component 'cargo'
2026-02-18T12:49:08.2955610Z info: downloading component 'rustc'
2026-02-18T12:49:08.8056690Z info: removing previous version of component 'clippy'
2026-02-18T12:49:08.8077230Z info: removing previous version of component 'rustfmt'
2026-02-18T12:49:08.8092060Z info: removing previous version of component 'cargo'
2026-02-18T12:49:08.8162690Z info: removing previous version of component 'rust-std'
2026-02-18T12:49:08.8207690Z info: removing previous version of component 'rustc'
2026-02-18T12:49:08.8285260Z info: installing component 'rust-std'
2026-02-18T12:49:10.4457650Z info: installing component 'clippy'
2026-02-18T12:49:10.7093430Z info: installing component 'rustfmt'
2026-02-18T12:49:10.9609760Z info: installing component 'cargo'
2026-02-18T12:49:11.5939310Z info: installing component 'rustc'
2026-02-18T12:49:15.2446230Z 
2026-02-18T12:49:15.4434590Z   stable-aarch64-apple-darwin updated - rustc 1.93.1 (01f6ddf75 2026-02-11) (from rustc 1.93.0 (254b59607 2026-01-19))
2026-02-18T12:49:15.4435070Z 
2026-02-18T12:49:15.4440420Z info: self-update is disabled for this build of rustup
2026-02-18T12:49:15.4441280Z info: any updates to rustup will need to be fetched with your system package manager
2026-02-18T12:49:15.4600860Z ##[group]Run rustup default stable
2026-02-18T12:49:15.4601350Z [36;1mrustup default stable[0m
2026-02-18T12:49:15.4634940Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.4635220Z env:
2026-02-18T12:49:15.4635540Z   DEBUG: napi:*
2026-02-18T12:49:15.4635690Z   APP_NAME: pdfdown
2026-02-18T12:49:15.4636060Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.4636270Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.4641250Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.4641590Z ##[endgroup]
2026-02-18T12:49:15.5091250Z info: using existing install for 'stable-aarch64-apple-darwin'
2026-02-18T12:49:15.5281600Z info: default toolchain set to 'stable-aarch64-apple-darwin'
2026-02-18T12:49:15.5281850Z 
2026-02-18T12:49:15.5379440Z   stable-aarch64-apple-darwin unchanged - rustc 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:49:15.5379770Z 
2026-02-18T12:49:15.5407430Z ##[group]Run : create cachekey
2026-02-18T12:49:15.5407670Z [36;1m: create cachekey[0m
2026-02-18T12:49:15.5408120Z [36;1mDATE=$(rustc +stable --version --verbose | sed -ne 's/^commit-date: \(20[0-9][0-9]\)-\([01][0-9]\)-\([0-3][0-9]\)$/\1\2\3/p')[0m
2026-02-18T12:49:15.5408640Z [36;1mHASH=$(rustc +stable --version --verbose | sed -ne 's/^commit-hash: //p')[0m
2026-02-18T12:49:15.5409090Z [36;1mecho "cachekey=$(echo $DATE$HASH | head -c12)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:49:15.5437830Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.5438250Z env:
2026-02-18T12:49:15.5438570Z   DEBUG: napi:*
2026-02-18T12:49:15.5438720Z   APP_NAME: pdfdown
2026-02-18T12:49:15.5438870Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.5439110Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.5439330Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.5439510Z ##[endgroup]
2026-02-18T12:49:15.6171670Z ##[group]Run : disable incremental compilation
2026-02-18T12:49:15.6171980Z [36;1m: disable incremental compilation[0m
2026-02-18T12:49:15.6172230Z [36;1mif [ -z "${CARGO_INCREMENTAL+set}" ]; then[0m
2026-02-18T12:49:15.6172540Z [36;1m  echo CARGO_INCREMENTAL=0 >> $GITHUB_ENV[0m
2026-02-18T12:49:15.6172830Z [36;1mfi[0m
2026-02-18T12:49:15.6205020Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.6205250Z env:
2026-02-18T12:49:15.6205440Z   DEBUG: napi:*
2026-02-18T12:49:15.6205750Z   APP_NAME: pdfdown
2026-02-18T12:49:15.6205900Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.6206140Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.6206410Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.6206640Z ##[endgroup]
2026-02-18T12:49:15.6447550Z ##[group]Run : enable colors in Cargo output
2026-02-18T12:49:15.6447920Z [36;1m: enable colors in Cargo output[0m
2026-02-18T12:49:15.6448160Z [36;1mif [ -z "${CARGO_TERM_COLOR+set}" ]; then[0m
2026-02-18T12:49:15.6448410Z [36;1m  echo CARGO_TERM_COLOR=always >> $GITHUB_ENV[0m
2026-02-18T12:49:15.6448690Z [36;1mfi[0m
2026-02-18T12:49:15.6490320Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.6490660Z env:
2026-02-18T12:49:15.6490910Z   DEBUG: napi:*
2026-02-18T12:49:15.6491120Z   APP_NAME: pdfdown
2026-02-18T12:49:15.6491340Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.6491630Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.6491800Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.6492020Z ##[endgroup]
2026-02-18T12:49:15.6693000Z ##[group]Run : enable Cargo sparse registry
2026-02-18T12:49:15.6693260Z [36;1m: enable Cargo sparse registry[0m
2026-02-18T12:49:15.6693560Z [36;1m# implemented in 1.66, stabilized in 1.68, made default in 1.70[0m
2026-02-18T12:49:15.6694120Z [36;1mif [ -z "${CARGO_REGISTRIES_CRATES_IO_PROTOCOL+set}" -o -f "/Users/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol ]; then[0m
2026-02-18T12:49:15.6694700Z [36;1m  if rustc +stable --version --verbose | grep -q '^release: 1\.6[89]\.'; then[0m
2026-02-18T12:49:15.6695170Z [36;1m    touch "/Users/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol || true[0m
2026-02-18T12:49:15.6695590Z [36;1m    echo CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse >> $GITHUB_ENV[0m
2026-02-18T12:49:15.6696050Z [36;1m  elif rustc +stable --version --verbose | grep -q '^release: 1\.6[67]\.'; then[0m
2026-02-18T12:49:15.6696520Z [36;1m    touch "/Users/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol || true[0m
2026-02-18T12:49:15.6696930Z [36;1m    echo CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git >> $GITHUB_ENV[0m
2026-02-18T12:49:15.6697390Z [36;1m  fi[0m
2026-02-18T12:49:15.6697540Z [36;1mfi[0m
2026-02-18T12:49:15.6735590Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.6736180Z env:
2026-02-18T12:49:15.6736350Z   DEBUG: napi:*
2026-02-18T12:49:15.6736520Z   APP_NAME: pdfdown
2026-02-18T12:49:15.6736690Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.6736910Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.6737090Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.6737340Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:15.6737510Z ##[endgroup]
2026-02-18T12:49:15.7380770Z ##[group]Run : work around spurious network errors in curl 8.0
2026-02-18T12:49:15.7381160Z [36;1m: work around spurious network errors in curl 8.0[0m
2026-02-18T12:49:15.7381610Z [36;1m# https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo/topic/timeout.20investigation[0m
2026-02-18T12:49:15.7382120Z [36;1mif rustc +stable --version --verbose | grep -q '^release: 1\.7[01]\.'; then[0m
2026-02-18T12:49:15.7382540Z [36;1m  echo CARGO_HTTP_MULTIPLEXING=false >> $GITHUB_ENV[0m
2026-02-18T12:49:15.7382770Z [36;1mfi[0m
2026-02-18T12:49:15.7417260Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.7417570Z env:
2026-02-18T12:49:15.7417700Z   DEBUG: napi:*
2026-02-18T12:49:15.7418020Z   APP_NAME: pdfdown
2026-02-18T12:49:15.7418190Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.7418370Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.7418550Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.7418730Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:15.7418890Z ##[endgroup]
2026-02-18T12:49:15.7798190Z ##[group]Run rustc +stable --version --verbose
2026-02-18T12:49:15.7798500Z [36;1mrustc +stable --version --verbose[0m
2026-02-18T12:49:15.7833630Z shell: /bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:49:15.7833930Z env:
2026-02-18T12:49:15.7834110Z   DEBUG: napi:*
2026-02-18T12:49:15.7834300Z   APP_NAME: pdfdown
2026-02-18T12:49:15.7834530Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.7834800Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.7835040Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.7835310Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:15.7835560Z ##[endgroup]
2026-02-18T12:49:15.8180950Z rustc 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:49:15.8181460Z binary: rustc
2026-02-18T12:49:15.8181810Z commit-hash: 01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf
2026-02-18T12:49:15.8182140Z commit-date: 2026-02-11
2026-02-18T12:49:15.8182420Z host: aarch64-apple-darwin
2026-02-18T12:49:15.8182680Z release: 1.93.1
2026-02-18T12:49:15.8185740Z LLVM version: 21.1.8
2026-02-18T12:49:15.8256420Z ##[group]Run actions/cache@v5
2026-02-18T12:49:15.8256630Z with:
2026-02-18T12:49:15.8256940Z   path: ~/.cargo/registry/index/
~/.cargo/registry/cache/
~/.cargo/git/db/
~/.napi-rs
.cargo-cache
target/

2026-02-18T12:49:15.8257350Z   key: ocr-aarch64-apple-darwin-cargo-macos-latest
2026-02-18T12:49:15.8257600Z   enableCrossOsArchive: false
2026-02-18T12:49:15.8257780Z   fail-on-cache-miss: false
2026-02-18T12:49:15.8257950Z   lookup-only: false
2026-02-18T12:49:15.8258110Z   save-always: false
2026-02-18T12:49:15.8258260Z env:
2026-02-18T12:49:15.8258420Z   DEBUG: napi:*
2026-02-18T12:49:15.8258550Z   APP_NAME: pdfdown
2026-02-18T12:49:15.8258710Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:15.8258900Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:15.8259060Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:15.8259240Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:15.8259410Z ##[endgroup]
2026-02-18T12:49:16.1032170Z Cache hit for: ocr-aarch64-apple-darwin-cargo-macos-latest
2026-02-18T12:49:17.2759550Z Received 79691776 of 223940918 (35.6%), 75.8 MBs/sec
2026-02-18T12:49:18.2775640Z Received 134217728 of 223940918 (59.9%), 63.9 MBs/sec
2026-02-18T12:49:18.8235150Z Received 223940918 of 223940918 (100.0%), 83.8 MBs/sec
2026-02-18T12:49:18.8325170Z Cache Size: ~214 MB (223940918 B)
2026-02-18T12:49:18.8327800Z [command]/opt/homebrew/bin/gtar -xf /Users/runner/work/_temp/0d800225-f5e2-4f5e-b467-a256dd7508b0/cache.tzst -P -C /Users/runner/work/pdfdown/pdfdown --delay-directory-restore --use-compress-program unzstd
2026-02-18T12:49:20.5351780Z Cache restored successfully
2026-02-18T12:49:20.5636730Z Cache restored from key: ocr-aarch64-apple-darwin-cargo-macos-latest
2026-02-18T12:49:20.5748940Z ##[group]Run brew update
2026-02-18T12:49:20.5749190Z [36;1mbrew update[0m
2026-02-18T12:49:20.5749420Z [36;1mbrew install tesseract[0m
2026-02-18T12:49:20.5749740Z [36;1mtesseract --version[0m
2026-02-18T12:49:20.5785120Z shell: /bin/bash -e {0}
2026-02-18T12:49:20.5785380Z env:
2026-02-18T12:49:20.5785540Z   DEBUG: napi:*
2026-02-18T12:49:20.5785780Z   APP_NAME: pdfdown
2026-02-18T12:49:20.5785970Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:20.5786210Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:20.5786440Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:20.5786670Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:20.5786840Z ##[endgroup]
2026-02-18T12:49:22.6323970Z [34m==>[0m [1mUpdating Homebrew...[0m
2026-02-18T12:49:34.2674910Z To restore the stashed changes to /opt/homebrew/Library/Taps/homebrew/homebrew-core, run:
2026-02-18T12:49:34.2675670Z   cd /opt/homebrew/Library/Taps/homebrew/homebrew-core && git stash pop
2026-02-18T12:49:35.3812090Z [34m==>[0m [1mHomebrew's analytics have entirely moved to our InfluxDB instance in the EU.[0m
2026-02-18T12:49:35.3812920Z We gather less data than before and have destroyed all Google Analytics data:
2026-02-18T12:49:35.3813700Z   [4mhttps://docs.brew.sh/Analytics[24m[0m
2026-02-18T12:49:35.3814260Z Please reconsider re-enabling analytics to help our volunteer maintainers with:
2026-02-18T12:49:35.3814810Z   brew analytics on
2026-02-18T12:49:35.3995630Z [34m==>[0m [1mHomebrew is run entirely by unpaid volunteers. Please consider donating:[0m
2026-02-18T12:49:35.3996390Z   [4mhttps://github.com/Homebrew/brew#donations[24m
2026-02-18T12:49:35.3996720Z 
2026-02-18T12:49:36.8902280Z Updated 4 taps (hashicorp/tap, aws/tap, homebrew/core and homebrew/cask).
2026-02-18T12:49:36.9014230Z [34m==>[0m [1mNew Formulae[0m
2026-02-18T12:49:37.2698460Z aoe: Terminal session manager for AI coding agents
2026-02-18T12:49:37.2699180Z bagel: CLI to audit posture and evaluate compromise blast radius
2026-02-18T12:49:37.2699750Z bazel@8: Google's own build tool
2026-02-18T12:49:37.2701980Z difi: Pixel-perfect terminal diff viewer
2026-02-18T12:49:37.2707960Z go@1.25: Open source programming language to build simple/reliable/efficient software
2026-02-18T12:49:37.2710070Z grafanactl: CLI to interact with Grafana
2026-02-18T12:49:37.2712230Z happy-coder: CLI for operating AI coding agents from mobile devices
2026-02-18T12:49:37.2719310Z libnpupnp: C++ base UPnP library, derived from Portable UPnP, a.k.a libupnp
2026-02-18T12:49:37.2722950Z libupnpp: C++ wrapper for libnpupnp
2026-02-18T12:49:37.2726580Z likec4: Architecture modeling tool with live diagrams from code
2026-02-18T12:49:37.2729150Z livereload: Local web server in Python
2026-02-18T12:49:37.2746530Z ls-hpack: HTTP/2 HPACK header compression library
2026-02-18T12:49:37.2749820Z mipsel-linux-gnu-binutils: GNU Binutils for mipsel-linux-gnu cross development
2026-02-18T12:49:37.2750960Z pcapmirror: Tool for capturing network traffic on remote host using TZSP or ERSPAN
2026-02-18T12:49:37.2769110Z picoruby: Smallest Ruby implementation for microcontrollers
2026-02-18T12:49:37.2769750Z rtk: CLI proxy to minimize LLM token consumption
2026-02-18T12:49:37.2770230Z run-kit: Universal multi-language runner and smart REPL
2026-02-18T12:49:37.2771250Z rustledger: Fast, pure Rust implementation of Beancount double-entry accounting
2026-02-18T12:49:37.2777420Z rustypaste: Minimal file upload/pastebin service
2026-02-18T12:49:37.2786520Z sss-cli: Shamir secret share command-line interface
2026-02-18T12:49:37.2826070Z tuckr: Super powered replacement for GNU Stow
2026-02-18T12:49:37.2827200Z umoci: Reference OCI implementation for creating, modifying and inspecting images
2026-02-18T12:49:37.2827900Z whodb-cli: Database management CLI with TUI interface, MCP server support, AI, and more
2026-02-18T12:49:37.2832270Z zxing-cpp: Multi-format barcode image processing library written in C++
2026-02-18T12:49:37.2848960Z [34m==>[0m [1mNew Casks[0m
2026-02-18T12:49:37.4812230Z codexmonitor: Monitor Codex activity
2026-02-18T12:49:37.4913750Z desktop-composer: Appearance manager for the system and individual applications
2026-02-18T12:49:37.4948380Z font-allkin
2026-02-18T12:49:37.4948820Z font-bpmf-huninn
2026-02-18T12:49:37.4949100Z font-bpmf-iansui
2026-02-18T12:49:37.4949380Z font-bpmf-zihi-kai-std
2026-02-18T12:49:37.4950030Z iloader: iOS Sideloading Companion
2026-02-18T12:49:37.4951270Z macpulse: System monitoring dashboard with historical analytics
2026-02-18T12:49:37.4951880Z mindwtr: Local-first GTD productivity tool
2026-02-18T12:49:37.4952310Z netviews: Network and Wi-Fi diagnostic tool
2026-02-18T12:49:37.4952980Z nugget: Customise your iOS device with animated wallpapers, disable daemons and more
2026-02-18T12:49:37.4954340Z opencomic: Comic and Manga reader
2026-02-18T12:49:37.4954860Z supacode: Native terminal coding agents command center
2026-02-18T12:49:37.4955240Z thaw@beta: Menu bar manager
2026-02-18T12:49:37.4956500Z thecommander: Dual-panel file manager inspired by Total Commander
2026-02-18T12:49:37.4957240Z threema-work@beta: End-to-end encrypted instant messaging application
2026-02-18T12:49:37.4958610Z updatest: Utility that shows the latest app updates
2026-02-18T12:49:37.9463540Z [34m==>[0m [1mOutdated Formulae[0m
2026-02-18T12:49:37.9464180Z aws-sam-cli
2026-02-18T12:49:37.9464560Z cryptography
2026-02-18T12:49:37.9465050Z freetype
2026-02-18T12:49:37.9465350Z gcc
2026-02-18T12:49:37.9465700Z git
2026-02-18T12:49:37.9465970Z glib
2026-02-18T12:49:37.9466310Z gnutls
2026-02-18T12:49:37.9466590Z libpng
2026-02-18T12:49:37.9466960Z libpq
2026-02-18T12:49:37.9467230Z libuv
2026-02-18T12:49:37.9467660Z yq
2026-02-18T12:49:37.9468130Z [34m==>[0m [1mOutdated Casks[0m
2026-02-18T12:49:37.9468520Z session-manager-plugin
2026-02-18T12:49:37.9468790Z 
2026-02-18T12:49:37.9469370Z You have [1m11[0m outdated formulae and [1m1[0m outdated cask installed.
2026-02-18T12:49:37.9470090Z You can upgrade them with [1mbrew upgrade[0m
2026-02-18T12:49:37.9470650Z or list them with [1mbrew outdated[0m.
2026-02-18T12:49:39.7366130Z [32m==>[0m [1mFetching downloads for: [32mtesseract[39m[0m
2026-02-18T12:49:40.0916220Z ✔︎ Bottle Manifest tesseract (5.5.2)
2026-02-18T12:49:41.4663600Z ✔︎ Bottle Manifest openjpeg (2.5.4)
2026-02-18T12:49:41.4774700Z ✔︎ Bottle Manifest libpng (1.6.55)
2026-02-18T12:49:41.4886620Z ✔︎ Bottle libpng (1.6.55)
2026-02-18T12:49:41.5014010Z ✔︎ Bottle openjpeg (2.5.4)
2026-02-18T12:49:41.5114950Z ✔︎ Bottle Manifest webp (1.6.0)
2026-02-18T12:49:41.5218720Z ✔︎ Bottle webp (1.6.0)
2026-02-18T12:49:41.5313350Z ✔︎ Bottle Manifest leptonica (1.87.0)
2026-02-18T12:49:41.5353140Z ✔︎ Bottle leptonica (1.87.0)
2026-02-18T12:49:41.5379710Z ✔︎ Bottle Manifest libb2 (0.98.1)
2026-02-18T12:49:41.5426990Z ✔︎ Bottle libb2 (0.98.1)
2026-02-18T12:49:41.5533730Z ✔︎ Bottle Manifest libarchive (3.8.5_1)
2026-02-18T12:49:41.5647420Z ✔︎ Bottle Manifest fribidi (1.0.16)
2026-02-18T12:49:41.6517710Z ✔︎ Bottle fribidi (1.0.16)
2026-02-18T12:49:41.6723800Z ✔︎ Bottle Manifest libdatrie (0.2.14)
2026-02-18T12:49:41.6829160Z ✔︎ Bottle Manifest libthai (0.1.30)
2026-02-18T12:49:41.6932340Z ✔︎ Bottle libarchive (3.8.5_1)
2026-02-18T12:49:41.7700880Z ✔︎ Bottle libarchive (3.8.5_1)
2026-02-18T12:49:41.7701520Z ✔︎ Bottle libdatrie (0.2.14)
2026-02-18T12:49:41.9011130Z ✔︎ Bottle Manifest pango (1.57.0_2)
2026-02-18T12:49:41.9072850Z ✔︎ Bottle Manifest freetype (2.14.1_2)
2026-02-18T12:49:41.9617330Z ✔︎ Bottle libthai (0.1.30)
2026-02-18T12:49:41.9722490Z ✔︎ Bottle Manifest glib (2.86.4)
2026-02-18T12:49:42.0257270Z ✔︎ Bottle freetype (2.14.1_2)
2026-02-18T12:49:42.4484740Z ✔︎ Bottle glib (2.86.4)
2026-02-18T12:49:42.4485720Z ✔︎ Bottle pango (1.57.0_2)
2026-02-18T12:49:42.5733540Z ✔︎ Bottle tesseract (5.5.2)
2026-02-18T12:49:42.8439360Z [32m==>[0m [1mInstalling dependencies for tesseract: [32mopenjpeg[39m, [32mwebp[39m, [32mleptonica[39m, [32mlibb2[39m, [32mlibarchive[39m, [32mfribidi[39m, [32mlibdatrie[39m, [32mlibthai[39m and [32mpango[39m[0m
2026-02-18T12:49:42.8449010Z [32m==>[0m [1mInstalling tesseract dependency: [32mopenjpeg[39m[0m
2026-02-18T12:49:42.9033290Z [34m==>[0m [1mPouring openjpeg--2.5.4.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:44.9036320Z 🍺  /opt/homebrew/Cellar/openjpeg/2.5.4: 512 files, 14.7MB
2026-02-18T12:49:44.9041920Z [32m==>[0m [1mInstalling tesseract dependency: [32mwebp[39m[0m
2026-02-18T12:49:44.9515110Z [34m==>[0m [1mPouring webp--1.6.0.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:46.5390740Z 🍺  /opt/homebrew/Cellar/webp/1.6.0: 64 files, 2.6MB
2026-02-18T12:49:46.5395160Z [32m==>[0m [1mInstalling tesseract dependency: [32mleptonica[39m[0m
2026-02-18T12:49:46.6752530Z [32m==>[0m [1mInstalling leptonica dependency: [32mlibpng[39m[0m
2026-02-18T12:49:46.6768500Z [34m==>[0m [1mPouring libpng--1.6.55.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:47.7809080Z 🍺  /opt/homebrew/Cellar/libpng/1.6.55: 28 files, 1.4MB
2026-02-18T12:49:47.7814590Z [34m==>[0m [1mPouring leptonica--1.87.0.arm64_sequoia.bottle.1.tar.gz[0m
2026-02-18T12:49:49.5294090Z 🍺  /opt/homebrew/Cellar/leptonica/1.87.0: 55 files, 7.3MB
2026-02-18T12:49:49.5298490Z [32m==>[0m [1mInstalling tesseract dependency: [32mlibb2[39m[0m
2026-02-18T12:49:49.5318020Z [34m==>[0m [1mPouring libb2--0.98.1.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:50.6006790Z 🍺  /opt/homebrew/Cellar/libb2/0.98.1: 9 files, 132.0KB
2026-02-18T12:49:50.6012250Z [32m==>[0m [1mInstalling tesseract dependency: [32mlibarchive[39m[0m
2026-02-18T12:49:50.6308910Z [34m==>[0m [1mPouring libarchive--3.8.5_1.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:51.9323090Z 🍺  /opt/homebrew/Cellar/libarchive/3.8.5_1: 65 files, 4MB
2026-02-18T12:49:51.9325880Z [32m==>[0m [1mInstalling tesseract dependency: [32mfribidi[39m[0m
2026-02-18T12:49:51.9341440Z [34m==>[0m [1mPouring fribidi--1.0.16.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:53.1471320Z 🍺  /opt/homebrew/Cellar/fribidi/1.0.16: 68 files, 581.6KB
2026-02-18T12:49:53.1478680Z [32m==>[0m [1mInstalling tesseract dependency: [32mlibdatrie[39m[0m
2026-02-18T12:49:53.1516390Z [34m==>[0m [1mPouring libdatrie--0.2.14.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:54.3684650Z 🍺  /opt/homebrew/Cellar/libdatrie/0.2.14: 20 files, 313.5KB
2026-02-18T12:49:54.3689070Z [32m==>[0m [1mInstalling tesseract dependency: [32mlibthai[39m[0m
2026-02-18T12:49:54.3768050Z [34m==>[0m [1mPouring libthai--0.1.30.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:55.4069050Z 🍺  /opt/homebrew/Cellar/libthai/0.1.30: 30 files, 975.3KB
2026-02-18T12:49:55.4072530Z [32m==>[0m [1mInstalling tesseract dependency: [32mpango[39m[0m
2026-02-18T12:49:55.6352620Z [34m==>[0m [1mPouring pango--1.57.0_2.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:57.1289750Z 🍺  /opt/homebrew/Cellar/pango/1.57.0_2: 69 files, 3.8MB
2026-02-18T12:49:57.1292100Z [32m==>[0m [1mInstalling [32mtesseract[39m[0m
2026-02-18T12:49:57.1295430Z [34m==>[0m [1mPouring tesseract--5.5.2.arm64_sequoia.bottle.tar.gz[0m
2026-02-18T12:49:59.4939670Z [34m==>[0m [1mCaveats[0m
2026-02-18T12:49:59.4941140Z This formula contains only the "eng", "osd", and "snum" language data files.
2026-02-18T12:49:59.4943020Z If you need any other supported languages, run `brew install tesseract-lang`.
2026-02-18T12:49:59.4944870Z [34m==>[0m [1mSummary[0m
2026-02-18T12:49:59.4956220Z 🍺  /opt/homebrew/Cellar/tesseract/5.5.2: 75 files, 34.9MB
2026-02-18T12:49:59.5027850Z [32m==>[0m [1mCaveats[0m
2026-02-18T12:49:59.5028760Z [34m==>[0m [1mtesseract[0m
2026-02-18T12:49:59.5029750Z This formula contains only the "eng", "osd", and "snum" language data files.
2026-02-18T12:49:59.5030510Z If you need any other supported languages, run `brew install tesseract-lang`.
2026-02-18T12:49:59.5713250Z tesseract 5.5.2
2026-02-18T12:49:59.5713810Z  leptonica-1.87.0
2026-02-18T12:49:59.5714500Z   libgif 5.2.2 : libjpeg 8d (libjpeg-turbo 3.1.3) : libpng 1.6.55 : libtiff 4.7.1 : zlib 1.2.12 : libwebp 1.6.0 : libopenjp2 2.5.4
2026-02-18T12:49:59.5716180Z  Found NEON
2026-02-18T12:49:59.5722150Z  Found libarchive 3.8.5 zlib/1.2.12 liblzma/5.8.2 bz2lib/1.0.8 liblz4/1.10.0 libzstd/1.5.7 expat/expat_2.7.1 CommonCrypto/system libb2/system
2026-02-18T12:49:59.5723000Z  Found libcurl/8.7.1 SecureTransport (LibreSSL/3.3.6) zlib/1.2.12 nghttp2/1.64.0
2026-02-18T12:49:59.5790910Z ##[group]Run ARCH=$(uname -m)
2026-02-18T12:49:59.5791290Z [36;1mARCH=$(uname -m)[0m
2026-02-18T12:49:59.5791630Z [36;1mif [ "$ARCH" = "x86_64" ]; then[0m
2026-02-18T12:49:59.5791900Z [36;1m  PDFIUM_ARCH="mac-x64"[0m
2026-02-18T12:49:59.5792160Z [36;1melif [ "$ARCH" = "arm64" ]; then[0m
2026-02-18T12:49:59.5792550Z [36;1m  PDFIUM_ARCH="mac-arm64"[0m
2026-02-18T12:49:59.5792790Z [36;1mfi[0m
2026-02-18T12:49:59.5793220Z [36;1mcurl -L "https://github.com/nicksrandall/pdfium-lib/releases/latest/download/pdfium-${PDFIUM_ARCH}.tgz" -o pdfium.tgz[0m
2026-02-18T12:49:59.5793710Z [36;1mmkdir -p /tmp/pdfium[0m
2026-02-18T12:49:59.5793920Z [36;1mtar xzf pdfium.tgz -C /tmp/pdfium[0m
2026-02-18T12:49:59.5794280Z [36;1mecho "PDFIUM_LIBRARY_PATH=/tmp/pdfium/lib/libpdfium.dylib" >> $GITHUB_ENV[0m
2026-02-18T12:49:59.5961340Z shell: /bin/bash -e {0}
2026-02-18T12:49:59.5961610Z env:
2026-02-18T12:49:59.5961910Z   DEBUG: napi:*
2026-02-18T12:49:59.5962050Z   APP_NAME: pdfdown
2026-02-18T12:49:59.5962440Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:49:59.5962620Z   CARGO_INCREMENTAL: 1
2026-02-18T12:49:59.5962840Z   CARGO_HOME: /Users/runner/.cargo
2026-02-18T12:49:59.5963070Z   CARGO_TERM_COLOR: always
2026-02-18T12:49:59.5963330Z ##[endgroup]
2026-02-18T12:49:59.6575730Z   % Total    % Received % Xferd  Average Speed   Time    Time     Time  Current
2026-02-18T12:49:59.6577250Z                                  Dload  Upload   Total   Spent    Left  Speed
2026-02-18T12:49:59.6577890Z 
2026-02-18T12:49:59.8019830Z   0     0    0     0    0     0      0      0 --:--:-- --:--:-- --:--:--     0
2026-02-18T12:49:59.8022000Z 100     9  100     9    0     0     62      0 --:--:-- --:--:-- --:--:--    62
2026-02-18T12:49:59.8156420Z tar: Error opening archive: Unrecognized archive format
2026-02-18T12:49:59.8197470Z ##[error]Process completed with exit code 1.
2026-02-18T12:49:59.8528190Z Post job cleanup.
2026-02-18T12:50:00.1943860Z [command]/opt/homebrew/bin/git version
2026-02-18T12:50:00.2075590Z git version 2.52.0
2026-02-18T12:50:00.2380010Z Copying '/Users/runner/.gitconfig' to '/Users/runner/work/_temp/d36d4781-9821-46df-9128-8db6efd99e6f/.gitconfig'
2026-02-18T12:50:00.2382290Z Temporarily overriding HOME='/Users/runner/work/_temp/d36d4781-9821-46df-9128-8db6efd99e6f' before making global git config changes
2026-02-18T12:50:00.2388250Z Adding repository directory to the temporary git global config as a safe directory
2026-02-18T12:50:00.2390440Z [command]/opt/homebrew/bin/git config --global --add safe.directory /Users/runner/work/pdfdown/pdfdown
2026-02-18T12:50:00.2391700Z Removing SSH command configuration
2026-02-18T12:50:00.2392350Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp core\.sshCommand
2026-02-18T12:50:00.2394280Z [command]/opt/homebrew/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'core\.sshCommand' && git config --local --unset-all 'core.sshCommand' || :"
2026-02-18T12:50:00.3317570Z Removing HTTP extra header
2026-02-18T12:50:00.3321780Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp http\.https\:\/\/github\.com\/\.extraheader
2026-02-18T12:50:00.3427360Z [command]/opt/homebrew/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'http\.https\:\/\/github\.com\/\.extraheader' && git config --local --unset-all 'http.https://github.com/.extraheader' || :"
2026-02-18T12:50:00.4334260Z Removing includeIf entries pointing to credentials config files
2026-02-18T12:50:00.4339400Z [command]/opt/homebrew/bin/git config --local --name-only --get-regexp ^includeIf\.gitdir:
2026-02-18T12:50:00.4398170Z includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git.path
2026-02-18T12:50:00.4400880Z includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git/worktrees/*.path
2026-02-18T12:50:00.4401360Z includeif.gitdir:/github/workspace/.git.path
2026-02-18T12:50:00.4401840Z includeif.gitdir:/github/workspace/.git/worktrees/*.path
2026-02-18T12:50:00.4404350Z [command]/opt/homebrew/bin/git config --local --get-all includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git.path
2026-02-18T12:50:00.4462290Z /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4469810Z [command]/opt/homebrew/bin/git config --local --unset includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git.path /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4544520Z [command]/opt/homebrew/bin/git config --local --get-all includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git/worktrees/*.path
2026-02-18T12:50:00.4605530Z /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4612380Z [command]/opt/homebrew/bin/git config --local --unset includeif.gitdir:/Users/runner/work/pdfdown/pdfdown/.git/worktrees/*.path /Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4675800Z [command]/opt/homebrew/bin/git config --local --get-all includeif.gitdir:/github/workspace/.git.path
2026-02-18T12:50:00.4730680Z /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4749900Z [command]/opt/homebrew/bin/git config --local --unset includeif.gitdir:/github/workspace/.git.path /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.4884920Z [command]/opt/homebrew/bin/git config --local --get-all includeif.gitdir:/github/workspace/.git/worktrees/*.path
2026-02-18T12:50:00.4962630Z /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.5014530Z [command]/opt/homebrew/bin/git config --local --unset includeif.gitdir:/github/workspace/.git/worktrees/*.path /github/runner_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config
2026-02-18T12:50:00.5019780Z [command]/opt/homebrew/bin/git submodule foreach --recursive git config --local --show-origin --name-only --get-regexp remote.origin.url
2026-02-18T12:50:00.5985070Z Removing credentials config '/Users/runner/work/_temp/git-credentials-1e8d5aca-aae3-49c1-be1a-2ee555a07907.config'
2026-02-18T12:50:00.6096270Z Cleaning up orphan processes

```
