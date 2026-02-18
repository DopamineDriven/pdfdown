do we need to add a way for the [CI](./.github/workflows/CI.yml) to incorporate pdfium?

```log
2026-02-18T12:20:26.4491399Z Current runner version: '2.331.0'
2026-02-18T12:20:26.4515917Z ##[group]Runner Image Provisioner
2026-02-18T12:20:26.4516730Z Hosted Compute Agent
2026-02-18T12:20:26.4517329Z Version: 20260123.484
2026-02-18T12:20:26.4517992Z Commit: 6bd6555ca37d84114959e1c76d2c01448ff61c5d
2026-02-18T12:20:26.4518649Z Build Date: 2026-01-23T19:41:17Z
2026-02-18T12:20:26.4519348Z Worker ID: {fbe2bb7f-3289-494f-b7a2-52ede471d2ac}
2026-02-18T12:20:26.4520063Z Azure Region: westcentralus
2026-02-18T12:20:26.4520605Z ##[endgroup]
2026-02-18T12:20:26.4522034Z ##[group]Operating System
2026-02-18T12:20:26.4522635Z Ubuntu
2026-02-18T12:20:26.4523513Z 24.04.3
2026-02-18T12:20:26.4523943Z LTS
2026-02-18T12:20:26.4524492Z ##[endgroup]
2026-02-18T12:20:26.4524960Z ##[group]Runner Image
2026-02-18T12:20:26.4525510Z Image: ubuntu-24.04
2026-02-18T12:20:26.4526125Z Version: 20260209.23.1
2026-02-18T12:20:26.4527276Z Included Software: https://github.com/actions/runner-images/blob/ubuntu24/20260209.23/images/ubuntu/Ubuntu2404-Readme.md
2026-02-18T12:20:26.4528754Z Image Release: https://github.com/actions/runner-images/releases/tag/ubuntu24%2F20260209.23
2026-02-18T12:20:26.4529697Z ##[endgroup]
2026-02-18T12:20:26.4531265Z ##[group]GITHUB_TOKEN Permissions
2026-02-18T12:20:26.4533594Z Contents: read
2026-02-18T12:20:26.4534116Z Metadata: read
2026-02-18T12:20:26.4534724Z Packages: read
2026-02-18T12:20:26.4535189Z ##[endgroup]
2026-02-18T12:20:26.4537487Z Secret source: Actions
2026-02-18T12:20:26.4538241Z Prepare workflow directory
2026-02-18T12:20:26.4987975Z Prepare all required actions
2026-02-18T12:20:26.5025378Z Getting action download info
2026-02-18T12:20:26.9847590Z Download action repository 'actions/checkout@v6' (SHA:de0fac2e4500dabe0009e67214ff5f5447ce83dd)
2026-02-18T12:20:27.1350947Z Download action repository 'actions/setup-node@v6' (SHA:6044e13b5dc448c55e2357c09f80417699197238)
2026-02-18T12:20:27.2198733Z Download action repository 'dtolnay/rust-toolchain@stable' (SHA:631a55b12751854ce901bb631d5902ceb48146f7)
2026-02-18T12:20:27.6575051Z Download action repository 'actions/cache@v5' (SHA:cdf6c1fa76f9f475f3d7449005a359c84ca0f306)
2026-02-18T12:20:27.7654695Z Download action repository 'mlugg/setup-zig@v2' (SHA:d1434d08867e3ee9daa34448df10607b98908d29)
2026-02-18T12:20:29.7873321Z Download action repository 'taiki-e/install-action@v2' (SHA:70e00552f3196d9a4c7dde7c57ef4c4830d422dd)
2026-02-18T12:20:30.5674767Z Download action repository 'actions/upload-artifact@v6' (SHA:b7c566a772e6b6bfb58ed0dc250532a479d7789f)
2026-02-18T12:20:30.8127832Z Complete job name: stable - wasm32-wasip1-threads - node@22
2026-02-18T12:20:30.8788933Z ##[group]Run actions/checkout@v6
2026-02-18T12:20:30.8789526Z with:
2026-02-18T12:20:30.8789741Z   repository: DopamineDriven/pdfdown
2026-02-18T12:20:30.8790179Z   token: ***
2026-02-18T12:20:30.8790371Z   ssh-strict: true
2026-02-18T12:20:30.8790568Z   ssh-user: git
2026-02-18T12:20:30.8790768Z   persist-credentials: true
2026-02-18T12:20:30.8790994Z   clean: true
2026-02-18T12:20:30.8791194Z   sparse-checkout-cone-mode: true
2026-02-18T12:20:30.8791447Z   fetch-depth: 1
2026-02-18T12:20:30.8791654Z   fetch-tags: false
2026-02-18T12:20:30.8791847Z   show-progress: true
2026-02-18T12:20:30.8792054Z   lfs: false
2026-02-18T12:20:30.8792232Z   submodules: false
2026-02-18T12:20:30.8792431Z   set-safe-directory: true
2026-02-18T12:20:30.8792852Z env:
2026-02-18T12:20:30.8793203Z   DEBUG: napi:*
2026-02-18T12:20:30.8793405Z   APP_NAME: pdfdown
2026-02-18T12:20:30.8793618Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:30.8793864Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:30.8794098Z ##[endgroup]
2026-02-18T12:20:30.9734527Z Syncing repository: DopamineDriven/pdfdown
2026-02-18T12:20:30.9735849Z ##[group]Getting Git version info
2026-02-18T12:20:30.9736292Z Working directory is '/home/runner/work/pdfdown/pdfdown'
2026-02-18T12:20:30.9736847Z [command]/usr/bin/git version
2026-02-18T12:20:30.9737114Z git version 2.52.0
2026-02-18T12:20:30.9784278Z ##[endgroup]
2026-02-18T12:20:30.9798900Z Temporarily overriding HOME='/home/runner/work/_temp/e974d179-adf8-4210-b5bc-5906d5cabe4f' before making global git config changes
2026-02-18T12:20:30.9800587Z Adding repository directory to the temporary git global config as a safe directory
2026-02-18T12:20:30.9803913Z [command]/usr/bin/git config --global --add safe.directory /home/runner/work/pdfdown/pdfdown
2026-02-18T12:20:30.9844809Z Deleting the contents of '/home/runner/work/pdfdown/pdfdown'
2026-02-18T12:20:30.9848371Z ##[group]Initializing the repository
2026-02-18T12:20:30.9853511Z [command]/usr/bin/git init /home/runner/work/pdfdown/pdfdown
2026-02-18T12:20:30.9954997Z hint: Using 'master' as the name for the initial branch. This default branch name
2026-02-18T12:20:30.9955961Z hint: will change to "main" in Git 3.0. To configure the initial branch name
2026-02-18T12:20:30.9956807Z hint: to use in all of your new repositories, which will suppress this warning,
2026-02-18T12:20:30.9957437Z hint: call:
2026-02-18T12:20:30.9957727Z hint:
2026-02-18T12:20:30.9958142Z hint: 	git config --global init.defaultBranch <name>
2026-02-18T12:20:30.9958676Z hint:
2026-02-18T12:20:30.9959170Z hint: Names commonly chosen instead of 'master' are 'main', 'trunk' and
2026-02-18T12:20:30.9960045Z hint: 'development'. The just-created branch can be renamed via this command:
2026-02-18T12:20:30.9960711Z hint:
2026-02-18T12:20:30.9961055Z hint: 	git branch -m <name>
2026-02-18T12:20:30.9961452Z hint:
2026-02-18T12:20:30.9961988Z hint: Disable this message with "git config set advice.defaultBranchName false"
2026-02-18T12:20:30.9963300Z Initialized empty Git repository in /home/runner/work/pdfdown/pdfdown/.git/
2026-02-18T12:20:30.9974458Z [command]/usr/bin/git remote add origin https://github.com/DopamineDriven/pdfdown
2026-02-18T12:20:31.0011367Z ##[endgroup]
2026-02-18T12:20:31.0012013Z ##[group]Disabling automatic garbage collection
2026-02-18T12:20:31.0015596Z [command]/usr/bin/git config --local gc.auto 0
2026-02-18T12:20:31.0046560Z ##[endgroup]
2026-02-18T12:20:31.0047139Z ##[group]Setting up auth
2026-02-18T12:20:31.0048106Z Removing SSH command configuration
2026-02-18T12:20:31.0054774Z [command]/usr/bin/git config --local --name-only --get-regexp core\.sshCommand
2026-02-18T12:20:31.0086555Z [command]/usr/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'core\.sshCommand' && git config --local --unset-all 'core.sshCommand' || :"
2026-02-18T12:20:31.0413869Z Removing HTTP extra header
2026-02-18T12:20:31.0419380Z [command]/usr/bin/git config --local --name-only --get-regexp http\.https\:\/\/github\.com\/\.extraheader
2026-02-18T12:20:31.0454130Z [command]/usr/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'http\.https\:\/\/github\.com\/\.extraheader' && git config --local --unset-all 'http.https://github.com/.extraheader' || :"
2026-02-18T12:20:31.0687218Z Removing includeIf entries pointing to credentials config files
2026-02-18T12:20:31.0692633Z [command]/usr/bin/git config --local --name-only --get-regexp ^includeIf\.gitdir:
2026-02-18T12:20:31.0727724Z [command]/usr/bin/git submodule foreach --recursive git config --local --show-origin --name-only --get-regexp remote.origin.url
2026-02-18T12:20:31.0966191Z [command]/usr/bin/git config --file /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config http.https://github.com/.extraheader AUTHORIZATION: basic ***
2026-02-18T12:20:31.1003558Z [command]/usr/bin/git config --local includeIf.gitdir:/home/runner/work/pdfdown/pdfdown/.git.path /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:20:31.1034221Z [command]/usr/bin/git config --local includeIf.gitdir:/home/runner/work/pdfdown/pdfdown/.git/worktrees/*.path /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:20:31.1064964Z [command]/usr/bin/git config --local includeIf.gitdir:/github/workspace/.git.path /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:20:31.1096615Z [command]/usr/bin/git config --local includeIf.gitdir:/github/workspace/.git/worktrees/*.path /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:20:31.1122481Z ##[endgroup]
2026-02-18T12:20:31.1123324Z ##[group]Fetching the repository
2026-02-18T12:20:31.1131897Z [command]/usr/bin/git -c protocol.version=2 fetch --no-tags --prune --no-recurse-submodules --depth=1 origin +0d70d33895379c3f8737c18997fddf7902e9dd3e:refs/remotes/origin/main
2026-02-18T12:20:31.7859180Z From https://github.com/DopamineDriven/pdfdown
2026-02-18T12:20:31.7860130Z  * [new ref]         0d70d33895379c3f8737c18997fddf7902e9dd3e -> origin/main
2026-02-18T12:20:31.7905587Z [command]/usr/bin/git branch --list --remote origin/main
2026-02-18T12:20:31.7933185Z   origin/main
2026-02-18T12:20:31.7943722Z [command]/usr/bin/git rev-parse refs/remotes/origin/main
2026-02-18T12:20:31.7969872Z 0d70d33895379c3f8737c18997fddf7902e9dd3e
2026-02-18T12:20:31.7974242Z ##[endgroup]
2026-02-18T12:20:31.7974872Z ##[group]Determining the checkout info
2026-02-18T12:20:31.7975868Z ##[endgroup]
2026-02-18T12:20:31.7980490Z [command]/usr/bin/git sparse-checkout disable
2026-02-18T12:20:31.8024269Z [command]/usr/bin/git config --local --unset-all extensions.worktreeConfig
2026-02-18T12:20:31.8052679Z ##[group]Checking out the ref
2026-02-18T12:20:31.8057380Z [command]/usr/bin/git checkout --progress --force -B main refs/remotes/origin/main
2026-02-18T12:20:31.8330566Z Switched to a new branch 'main'
2026-02-18T12:20:31.8334090Z branch 'main' set up to track 'origin/main'.
2026-02-18T12:20:31.8339965Z ##[endgroup]
2026-02-18T12:20:31.8380465Z [command]/usr/bin/git log -1 --format=%H
2026-02-18T12:20:31.8406333Z 0d70d33895379c3f8737c18997fddf7902e9dd3e
2026-02-18T12:20:31.8629328Z ##[group]Run actions/setup-node@v6
2026-02-18T12:20:31.8629615Z with:
2026-02-18T12:20:31.8629787Z   node-version: 24
2026-02-18T12:20:31.8629979Z   cache: yarn
2026-02-18T12:20:31.8630162Z   check-latest: false
2026-02-18T12:20:31.8630502Z   token: ***
2026-02-18T12:20:31.8630703Z   package-manager-cache: true
2026-02-18T12:20:31.8630936Z env:
2026-02-18T12:20:31.8631112Z   DEBUG: napi:*
2026-02-18T12:20:31.8631298Z   APP_NAME: pdfdown
2026-02-18T12:20:31.8631503Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:31.8631739Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:31.8631934Z ##[endgroup]
2026-02-18T12:20:31.9977420Z Found in cache @ /opt/hostedtoolcache/node/24.13.0/x64
2026-02-18T12:20:31.9980774Z ##[group]Environment details
2026-02-18T12:20:32.5764268Z node: v24.13.0
2026-02-18T12:20:32.5764698Z npm: 11.6.2
2026-02-18T12:20:32.5764999Z yarn: 4.12.0
2026-02-18T12:20:32.5765794Z ##[endgroup]
2026-02-18T12:20:32.5790096Z [command]/usr/local/bin/yarn --version
2026-02-18T12:20:32.9120477Z 4.12.0
2026-02-18T12:20:32.9310571Z [command]/usr/local/bin/yarn config get cacheFolder
2026-02-18T12:20:33.2637509Z /home/runner/.yarn/berry/cache
2026-02-18T12:20:33.2956881Z [command]/usr/local/bin/yarn config get enableGlobalCache
2026-02-18T12:20:33.6334302Z [33mtrue[39m
2026-02-18T12:20:33.8806272Z Cache hit for: node-cache-Linux-x64-yarn-c53c71cbec123083256fad35bd794fd7c2e247f2bc84f4bd923a8c5547d7cfc5
2026-02-18T12:20:35.0984412Z Received 33554432 of 37435410 (89.6%), 32.0 MBs/sec
2026-02-18T12:20:35.1717947Z Received 37435410 of 37435410 (100.0%), 33.2 MBs/sec
2026-02-18T12:20:35.1718684Z Cache Size: ~36 MB (37435410 B)
2026-02-18T12:20:35.1750649Z [command]/usr/bin/tar -xf /home/runner/work/_temp/6c12dd69-b55d-402c-9d6e-c700f8c7a17d/cache.tzst -P -C /home/runner/work/pdfdown/pdfdown --use-compress-program unzstd
2026-02-18T12:20:35.4404179Z Cache restored successfully
2026-02-18T12:20:35.4480403Z Cache restored from key: node-cache-Linux-x64-yarn-c53c71cbec123083256fad35bd794fd7c2e247f2bc84f4bd923a8c5547d7cfc5
2026-02-18T12:20:35.4754370Z ##[group]Run dtolnay/rust-toolchain@stable
2026-02-18T12:20:35.4754701Z with:
2026-02-18T12:20:35.4754888Z   toolchain: stable
2026-02-18T12:20:35.4755123Z   targets: wasm32-wasip1-threads
2026-02-18T12:20:35.4755543Z env:
2026-02-18T12:20:35.4755710Z   DEBUG: napi:*
2026-02-18T12:20:35.4755916Z   APP_NAME: pdfdown
2026-02-18T12:20:35.4756138Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.4756392Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.4756604Z ##[endgroup]
2026-02-18T12:20:35.4856972Z ##[group]Run : parse toolchain version
2026-02-18T12:20:35.4857341Z [36;1m: parse toolchain version[0m
2026-02-18T12:20:35.4857646Z [36;1mif [[ -z $toolchain ]]; then[0m
2026-02-18T12:20:35.4858204Z [36;1m  # GitHub does not enforce `required: true` inputs itself. https://github.com/actions/runner/issues/1070[0m
2026-02-18T12:20:35.4858763Z [36;1m  echo "'toolchain' is a required input" >&2[0m
2026-02-18T12:20:35.4859054Z [36;1m  exit 1[0m
2026-02-18T12:20:35.4859420Z [36;1melif [[ $toolchain =~ ^stable' '[0-9]+' '(year|month|week|day)s?' 'ago$ ]]; then[0m
2026-02-18T12:20:35.4859828Z [36;1m  if [[ Linux == macOS ]]; then[0m
2026-02-18T12:20:35.4860358Z [36;1m    echo "toolchain=1.$((($(date -v-$(sed 's/stable \([0-9]*\) \(.\).*/\1\2/' <<< $toolchain) +%s)/60/60/24-16569)/7/6))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.4860880Z [36;1m  else[0m
2026-02-18T12:20:35.4861275Z [36;1m    echo "toolchain=1.$((($(date --date "${toolchain#stable }" +%s)/60/60/24-16569)/7/6))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.4861741Z [36;1m  fi[0m
2026-02-18T12:20:35.4862035Z [36;1melif [[ $toolchain =~ ^stable' 'minus' '[0-9]+' 'releases?$ ]]; then[0m
2026-02-18T12:20:35.4862573Z [36;1m  echo "toolchain=1.$((($(date +%s)/60/60/24-16569)/7/6-${toolchain//[^0-9]/}))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.4863256Z [36;1melif [[ $toolchain =~ ^1\.[0-9]+$ ]]; then[0m
2026-02-18T12:20:35.4863783Z [36;1m  echo "toolchain=1.$((i=${toolchain#1.}, c=($(date +%s)/60/60/24-16569)/7/6, i+9*i*(10*i<=c)+90*i*(100*i<=c)))" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.4864277Z [36;1melse[0m
2026-02-18T12:20:35.4864525Z [36;1m  echo "toolchain=$toolchain" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.4864817Z [36;1mfi[0m
2026-02-18T12:20:35.4924344Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:35.4924705Z env:
2026-02-18T12:20:35.4924886Z   DEBUG: napi:*
2026-02-18T12:20:35.4925098Z   APP_NAME: pdfdown
2026-02-18T12:20:35.4925322Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.4925571Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.4925780Z   toolchain: stable
2026-02-18T12:20:35.4925978Z ##[endgroup]
2026-02-18T12:20:35.5066666Z ##[group]Run : construct rustup command line
2026-02-18T12:20:35.5066993Z [36;1m: construct rustup command line[0m
2026-02-18T12:20:35.5067439Z [36;1mecho "targets=$(for t in ${targets//,/ }; do echo -n ' --target' $t; done)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.5068080Z [36;1mecho "components=$(for c in ${components//,/ }; do echo -n ' --component' $c; done)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.5068587Z [36;1mecho "downgrade=" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:35.5114534Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:35.5114887Z env:
2026-02-18T12:20:35.5115069Z   DEBUG: napi:*
2026-02-18T12:20:35.5115272Z   APP_NAME: pdfdown
2026-02-18T12:20:35.5115481Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.5115748Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.5116000Z   targets: wasm32-wasip1-threads
2026-02-18T12:20:35.5116233Z   components: 
2026-02-18T12:20:35.5116421Z ##[endgroup]
2026-02-18T12:20:35.5213975Z ##[group]Run : set $CARGO_HOME
2026-02-18T12:20:35.5214238Z [36;1m: set $CARGO_HOME[0m
2026-02-18T12:20:35.5214561Z [36;1mecho CARGO_HOME=${CARGO_HOME:-"$HOME/.cargo"} >> $GITHUB_ENV[0m
2026-02-18T12:20:35.5258044Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:35.5258390Z env:
2026-02-18T12:20:35.5258567Z   DEBUG: napi:*
2026-02-18T12:20:35.5258971Z   APP_NAME: pdfdown
2026-02-18T12:20:35.5259201Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.5259470Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.5259679Z ##[endgroup]
2026-02-18T12:20:35.5357866Z ##[group]Run : install rustup if needed
2026-02-18T12:20:35.5358320Z [36;1m: install rustup if needed[0m
2026-02-18T12:20:35.5358620Z [36;1mif ! command -v rustup &>/dev/null; then[0m
2026-02-18T12:20:35.5359389Z [36;1m  curl --proto '=https' --tlsv1.2 --retry 10 --retry-connrefused --location --silent --show-error --fail https://sh.rustup.rs | sh -s -- --default-toolchain none -y[0m
2026-02-18T12:20:35.5360124Z [36;1m  echo "$CARGO_HOME/bin" >> $GITHUB_PATH[0m
2026-02-18T12:20:35.5360405Z [36;1mfi[0m
2026-02-18T12:20:35.5404393Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:35.5404753Z env:
2026-02-18T12:20:35.5404935Z   DEBUG: napi:*
2026-02-18T12:20:35.5405132Z   APP_NAME: pdfdown
2026-02-18T12:20:35.5405355Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.5405634Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.5405847Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:35.5406082Z ##[endgroup]
2026-02-18T12:20:35.5500950Z ##[group]Run rustup toolchain install stable --target wasm32-wasip1-threads --profile minimal --no-self-update
2026-02-18T12:20:35.5501738Z [36;1mrustup toolchain install stable --target wasm32-wasip1-threads --profile minimal --no-self-update[0m
2026-02-18T12:20:35.5545383Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:35.5545732Z env:
2026-02-18T12:20:35.5545913Z   DEBUG: napi:*
2026-02-18T12:20:35.5546113Z   APP_NAME: pdfdown
2026-02-18T12:20:35.5546340Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:35.5546591Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:35.5546811Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:35.5547047Z   RUSTUP_PERMIT_COPY_RENAME: 1
2026-02-18T12:20:35.5547274Z ##[endgroup]
2026-02-18T12:20:37.6160096Z info: syncing channel updates for 'stable-x86_64-unknown-linux-gnu'
2026-02-18T12:20:37.8586698Z info: latest update on 2026-02-12, rust version 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:20:37.9094821Z info: downloading component 'rust-std' for 'wasm32-wasip1-threads'
2026-02-18T12:20:38.3079888Z info: downloading component 'clippy'
2026-02-18T12:20:38.4850361Z info: downloading component 'rustfmt'
2026-02-18T12:20:38.6347474Z info: downloading component 'cargo'
2026-02-18T12:20:38.8865553Z info: downloading component 'rust-std'
2026-02-18T12:20:39.3561004Z info: downloading component 'rustc'
2026-02-18T12:20:40.3489844Z info: removing previous version of component 'clippy'
2026-02-18T12:20:40.3561483Z info: removing previous version of component 'rustfmt'
2026-02-18T12:20:40.3573627Z info: removing previous version of component 'cargo'
2026-02-18T12:20:40.3675324Z info: removing previous version of component 'rust-std'
2026-02-18T12:20:40.3705372Z info: removing previous version of component 'rustc'
2026-02-18T12:20:40.3810714Z info: installing component 'rust-std' for 'wasm32-wasip1-threads'
2026-02-18T12:20:41.8512011Z info: installing component 'clippy'
2026-02-18T12:20:42.2267292Z info: installing component 'rustfmt'
2026-02-18T12:20:42.4572443Z info: installing component 'cargo'
2026-02-18T12:20:43.1549709Z info: installing component 'rust-std'
2026-02-18T12:20:45.0507381Z info: installing component 'rustc'
2026-02-18T12:20:49.6295644Z 
2026-02-18T12:20:49.6394950Z   stable-x86_64-unknown-linux-gnu updated - rustc 1.93.1 (01f6ddf75 2026-02-11) (from rustc 1.93.0 (254b59607 2026-01-19))
2026-02-18T12:20:49.6395498Z 
2026-02-18T12:20:49.6444051Z ##[group]Run rustup default stable
2026-02-18T12:20:49.6444337Z [36;1mrustup default stable[0m
2026-02-18T12:20:49.6493294Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.6493625Z env:
2026-02-18T12:20:49.6493804Z   DEBUG: napi:*
2026-02-18T12:20:49.6494003Z   APP_NAME: pdfdown
2026-02-18T12:20:49.6494216Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.6494466Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.6494872Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.6495106Z ##[endgroup]
2026-02-18T12:20:49.6616981Z info: using existing install for 'stable-x86_64-unknown-linux-gnu'
2026-02-18T12:20:49.6918296Z info: default toolchain set to 'stable-x86_64-unknown-linux-gnu'
2026-02-18T12:20:49.6919238Z 
2026-02-18T12:20:49.7011308Z   stable-x86_64-unknown-linux-gnu unchanged - rustc 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:20:49.7011728Z 
2026-02-18T12:20:49.7054353Z ##[group]Run : create cachekey
2026-02-18T12:20:49.7054639Z [36;1m: create cachekey[0m
2026-02-18T12:20:49.7055139Z [36;1mDATE=$(rustc +stable --version --verbose | sed -ne 's/^commit-date: \(20[0-9][0-9]\)-\([01][0-9]\)-\([0-3][0-9]\)$/\1\2\3/p')[0m
2026-02-18T12:20:49.7055803Z [36;1mHASH=$(rustc +stable --version --verbose | sed -ne 's/^commit-hash: //p')[0m
2026-02-18T12:20:49.7056303Z [36;1mecho "cachekey=$(echo $DATE$HASH | head -c12)" >> $GITHUB_OUTPUT[0m
2026-02-18T12:20:49.7104726Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.7105063Z env:
2026-02-18T12:20:49.7105242Z   DEBUG: napi:*
2026-02-18T12:20:49.7105433Z   APP_NAME: pdfdown
2026-02-18T12:20:49.7105653Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.7105903Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.7106135Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.7106361Z ##[endgroup]
2026-02-18T12:20:49.7527185Z ##[group]Run : disable incremental compilation
2026-02-18T12:20:49.7527547Z [36;1m: disable incremental compilation[0m
2026-02-18T12:20:49.7527868Z [36;1mif [ -z "${CARGO_INCREMENTAL+set}" ]; then[0m
2026-02-18T12:20:49.7528194Z [36;1m  echo CARGO_INCREMENTAL=0 >> $GITHUB_ENV[0m
2026-02-18T12:20:49.7528463Z [36;1mfi[0m
2026-02-18T12:20:49.7578331Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.7578667Z env:
2026-02-18T12:20:49.7578847Z   DEBUG: napi:*
2026-02-18T12:20:49.7579041Z   APP_NAME: pdfdown
2026-02-18T12:20:49.7579267Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.7579533Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.7579753Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.7579983Z ##[endgroup]
2026-02-18T12:20:49.7664989Z ##[group]Run : enable colors in Cargo output
2026-02-18T12:20:49.7665302Z [36;1m: enable colors in Cargo output[0m
2026-02-18T12:20:49.7665618Z [36;1mif [ -z "${CARGO_TERM_COLOR+set}" ]; then[0m
2026-02-18T12:20:49.7665943Z [36;1m  echo CARGO_TERM_COLOR=always >> $GITHUB_ENV[0m
2026-02-18T12:20:49.7666212Z [36;1mfi[0m
2026-02-18T12:20:49.7709335Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.7709667Z env:
2026-02-18T12:20:49.7709840Z   DEBUG: napi:*
2026-02-18T12:20:49.7710030Z   APP_NAME: pdfdown
2026-02-18T12:20:49.7710246Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.7710499Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.7710718Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.7710942Z ##[endgroup]
2026-02-18T12:20:49.7795807Z ##[group]Run : enable Cargo sparse registry
2026-02-18T12:20:49.7796133Z [36;1m: enable Cargo sparse registry[0m
2026-02-18T12:20:49.7796492Z [36;1m# implemented in 1.66, stabilized in 1.68, made default in 1.70[0m
2026-02-18T12:20:49.7797195Z [36;1mif [ -z "${CARGO_REGISTRIES_CRATES_IO_PROTOCOL+set}" -o -f "/home/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol ]; then[0m
2026-02-18T12:20:49.7797928Z [36;1m  if rustc +stable --version --verbose | grep -q '^release: 1\.6[89]\.'; then[0m
2026-02-18T12:20:49.7798495Z [36;1m    touch "/home/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol || true[0m
2026-02-18T12:20:49.7799028Z [36;1m    echo CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse >> $GITHUB_ENV[0m
2026-02-18T12:20:49.7799520Z [36;1m  elif rustc +stable --version --verbose | grep -q '^release: 1\.6[67]\.'; then[0m
2026-02-18T12:20:49.7800105Z [36;1m    touch "/home/runner/work/_temp"/.implicit_cargo_registries_crates_io_protocol || true[0m
2026-02-18T12:20:49.7800807Z [36;1m    echo CARGO_REGISTRIES_CRATES_IO_PROTOCOL=git >> $GITHUB_ENV[0m
2026-02-18T12:20:49.7801171Z [36;1m  fi[0m
2026-02-18T12:20:49.7801354Z [36;1mfi[0m
2026-02-18T12:20:49.7846331Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.7846664Z env:
2026-02-18T12:20:49.7846840Z   DEBUG: napi:*
2026-02-18T12:20:49.7847191Z   APP_NAME: pdfdown
2026-02-18T12:20:49.7847414Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.7847659Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.7847873Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.7848109Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:49.7848321Z ##[endgroup]
2026-02-18T12:20:49.8229269Z ##[group]Run : work around spurious network errors in curl 8.0
2026-02-18T12:20:49.8229695Z [36;1m: work around spurious network errors in curl 8.0[0m
2026-02-18T12:20:49.8230239Z [36;1m# https://rust-lang.zulipchat.com/#narrow/stream/246057-t-cargo/topic/timeout.20investigation[0m
2026-02-18T12:20:49.8230855Z [36;1mif rustc +stable --version --verbose | grep -q '^release: 1\.7[01]\.'; then[0m
2026-02-18T12:20:49.8231313Z [36;1m  echo CARGO_HTTP_MULTIPLEXING=false >> $GITHUB_ENV[0m
2026-02-18T12:20:49.8231612Z [36;1mfi[0m
2026-02-18T12:20:49.8279817Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.8280150Z env:
2026-02-18T12:20:49.8280339Z   DEBUG: napi:*
2026-02-18T12:20:49.8280535Z   APP_NAME: pdfdown
2026-02-18T12:20:49.8280754Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.8280997Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.8281202Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.8281433Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:49.8281641Z ##[endgroup]
2026-02-18T12:20:49.8538933Z ##[group]Run rustc +stable --version --verbose
2026-02-18T12:20:49.8539278Z [36;1mrustc +stable --version --verbose[0m
2026-02-18T12:20:49.8588727Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:49.8589063Z env:
2026-02-18T12:20:49.8589244Z   DEBUG: napi:*
2026-02-18T12:20:49.8589445Z   APP_NAME: pdfdown
2026-02-18T12:20:49.8589679Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.8589928Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.8590142Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.8590375Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:49.8590588Z ##[endgroup]
2026-02-18T12:20:49.8787972Z rustc 1.93.1 (01f6ddf75 2026-02-11)
2026-02-18T12:20:49.8789031Z binary: rustc
2026-02-18T12:20:49.8789911Z commit-hash: 01f6ddf7588f42ae2d7eb0a2f21d44e8e96674cf
2026-02-18T12:20:49.8790542Z commit-date: 2026-02-11
2026-02-18T12:20:49.8791025Z host: x86_64-unknown-linux-gnu
2026-02-18T12:20:49.8791443Z release: 1.93.1
2026-02-18T12:20:49.8791841Z LLVM version: 21.1.8
2026-02-18T12:20:49.8879943Z ##[group]Run actions/cache@v5
2026-02-18T12:20:49.8880195Z with:
2026-02-18T12:20:49.8880597Z   path: ~/.cargo/registry/index/
~/.cargo/registry/cache/
~/.cargo/git/db/
~/.napi-rs
.cargo-cache
target/

2026-02-18T12:20:49.8881122Z   key: wasm32-wasip1-threads-cargo-ubuntu-latest
2026-02-18T12:20:49.8881433Z   enableCrossOsArchive: false
2026-02-18T12:20:49.8881672Z   fail-on-cache-miss: false
2026-02-18T12:20:49.8881893Z   lookup-only: false
2026-02-18T12:20:49.8882091Z   save-always: false
2026-02-18T12:20:49.8882268Z env:
2026-02-18T12:20:49.8882432Z   DEBUG: napi:*
2026-02-18T12:20:49.8882626Z   APP_NAME: pdfdown
2026-02-18T12:20:49.8882825Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:49.8883342Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:49.8883564Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:49.8883799Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:49.8884010Z ##[endgroup]
2026-02-18T12:20:50.2148777Z Cache hit for: wasm32-wasip1-threads-cargo-ubuntu-latest
2026-02-18T12:20:51.4414416Z Received 41943040 of 110427592 (38.0%), 39.9 MBs/sec
2026-02-18T12:20:51.8360292Z Received 110427592 of 110427592 (100.0%), 75.2 MBs/sec
2026-02-18T12:20:51.8363662Z Cache Size: ~105 MB (110427592 B)
2026-02-18T12:20:51.8404393Z [command]/usr/bin/tar -xf /home/runner/work/_temp/ea25e421-ae7a-4079-b4a8-e344a5762b50/cache.tzst -P -C /home/runner/work/pdfdown/pdfdown --use-compress-program unzstd
2026-02-18T12:20:52.3471142Z Cache restored successfully
2026-02-18T12:20:52.3769664Z Cache restored from key: wasm32-wasip1-threads-cargo-ubuntu-latest
2026-02-18T12:20:52.3929100Z ##[group]Run yarn install
2026-02-18T12:20:52.3929554Z [36;1myarn install[0m
2026-02-18T12:20:52.3979371Z shell: /usr/bin/bash -e {0}
2026-02-18T12:20:52.3979636Z env:
2026-02-18T12:20:52.3979824Z   DEBUG: napi:*
2026-02-18T12:20:52.3980033Z   APP_NAME: pdfdown
2026-02-18T12:20:52.3980268Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:52.3980531Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:52.3980753Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:52.3981006Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:52.3981234Z ##[endgroup]
2026-02-18T12:20:52.7779563Z [94m➤[39m YN0000: · Yarn 4.12.0
2026-02-18T12:20:52.7979925Z [94m➤[39m [90mYN0000[39m: ┌ Resolution step
2026-02-18T12:20:52.7980875Z ##[group]Resolution step
2026-02-18T12:20:52.8978957Z ##[endgroup]
2026-02-18T12:20:52.8979739Z [94m➤[39m [90mYN0000[39m: └ Completed
2026-02-18T12:20:52.9127533Z [94m➤[39m [90mYN0000[39m: ┌ Fetch step
2026-02-18T12:20:52.9128102Z ##[group]Fetch step
2026-02-18T12:20:53.1819392Z ##[endgroup]
2026-02-18T12:20:53.1821214Z [94m➤[39m [90mYN0000[39m: └ Completed in 0s 269ms
2026-02-18T12:20:53.1907588Z [94m➤[39m [90mYN0000[39m: ┌ Link step
2026-02-18T12:20:53.1908219Z ##[group]Link step
2026-02-18T12:20:54.6648307Z ##[endgroup]
2026-02-18T12:20:54.6649386Z [94m➤[39m [90mYN0000[39m: └ Completed in 1s 474ms
2026-02-18T12:20:54.6855138Z [94m➤[39m YN0000: · Done in 1s 909ms
2026-02-18T12:20:54.7180602Z ##[group]Run yarn build --target wasm32-wasip1-threads
2026-02-18T12:20:54.7181027Z [36;1myarn build --target wasm32-wasip1-threads[0m
2026-02-18T12:20:54.7229848Z shell: /usr/bin/bash --noprofile --norc -e -o pipefail {0}
2026-02-18T12:20:54.7230196Z env:
2026-02-18T12:20:54.7230372Z   DEBUG: napi:*
2026-02-18T12:20:54.7230583Z   APP_NAME: pdfdown
2026-02-18T12:20:54.7230806Z   MACOSX_DEPLOYMENT_TARGET: 10.13
2026-02-18T12:20:54.7231061Z   CARGO_INCREMENTAL: 1
2026-02-18T12:20:54.7231289Z   CARGO_HOME: /home/runner/.cargo
2026-02-18T12:20:54.7231542Z   CARGO_TERM_COLOR: always
2026-02-18T12:20:54.7231758Z ##[endgroup]
2026-02-18T12:20:55.3528380Z 2026-02-18T12:20:55.352Z napi:build napi build command receive options: {
2026-02-18T12:20:55.3529157Z   target: 'wasm32-wasip1-threads',
2026-02-18T12:20:55.3529545Z   cwd: undefined,
2026-02-18T12:20:55.3529778Z   manifestPath: undefined,
2026-02-18T12:20:55.3530026Z   configPath: undefined,
2026-02-18T12:20:55.3530345Z   packageJsonPath: undefined,
2026-02-18T12:20:55.3530596Z   targetDir: undefined,
2026-02-18T12:20:55.3530822Z   outputDir: undefined,
2026-02-18T12:20:55.3531035Z   platform: true,
2026-02-18T12:20:55.3531248Z   jsPackageName: undefined,
2026-02-18T12:20:55.3531482Z   constEnum: undefined,
2026-02-18T12:20:55.3531698Z   jsBinding: undefined,
2026-02-18T12:20:55.3531922Z   noJsBinding: undefined,
2026-02-18T12:20:55.3532158Z   dts: undefined,
2026-02-18T12:20:55.3532370Z   dtsHeader: undefined,
2026-02-18T12:20:55.3532719Z   noDtsHeader: undefined,
2026-02-18T12:20:55.3533184Z   dtsCache: true,
2026-02-18T12:20:55.3533427Z   esm: undefined,
2026-02-18T12:20:55.3533627Z   strip: undefined,
2026-02-18T12:20:55.3533860Z   release: true,
2026-02-18T12:20:55.3534070Z   verbose: undefined,
2026-02-18T12:20:55.3534282Z   bin: undefined,
2026-02-18T12:20:55.3534486Z   package: undefined,
2026-02-18T12:20:55.3534700Z   profile: undefined,
2026-02-18T12:20:55.3534923Z   crossCompile: undefined,
2026-02-18T12:20:55.3535185Z   useCross: undefined,
2026-02-18T12:20:55.3535421Z   useNapiCross: undefined,
2026-02-18T12:20:55.3535645Z   watch: undefined,
2026-02-18T12:20:55.3535855Z   features: [ 'render' ],
2026-02-18T12:20:55.3536083Z   allFeatures: undefined,
2026-02-18T12:20:55.3536342Z   noDefaultFeatures: undefined,
2026-02-18T12:20:55.3536594Z   cargoOptions: []
2026-02-18T12:20:55.3536798Z }
2026-02-18T12:20:57.7781189Z 2026-02-18T12:20:57.777Z napi:build Set features flags: 
2026-02-18T12:20:57.7782119Z 2026-02-18T12:20:57.778Z napi:build   [ '--features', 'render' ]
2026-02-18T12:20:57.7782761Z 2026-02-18T12:20:57.778Z napi:build Set compiling target to: 
2026-02-18T12:20:57.7784261Z 2026-02-18T12:20:57.778Z napi:build   [32mwasm32-wasip1-threads[39m
2026-02-18T12:20:57.7798334Z 2026-02-18T12:20:57.779Z napi:build Set envs: 
2026-02-18T12:20:57.7800065Z 2026-02-18T12:20:57.779Z napi:build   [32mNAPI_TYPE_DEF_TMP_FOLDER=/home/runner/work/pdfdown/pdfdown/target/napi-rs/pdfdown-da403edc[39m
2026-02-18T12:20:57.7802379Z 2026-02-18T12:20:57.779Z napi:build   [32mEMNAPI_LINK_DIR=/home/runner/work/pdfdown/pdfdown/node_modules/emnapi/lib/wasm32-wasi-threads[39m
2026-02-18T12:20:57.7804075Z 2026-02-18T12:20:57.780Z napi:build Start building crate: pdfdown
2026-02-18T12:20:57.7805607Z 2026-02-18T12:20:57.780Z napi:build   [32mcargo build --features render --target wasm32-wasip1-threads --release[39m
2026-02-18T12:20:57.8930729Z [1m[92m Downloading[0m crates ...
2026-02-18T12:20:57.9836302Z [1m[92m  Downloaded[0m vecmath v1.0.0
2026-02-18T12:20:57.9848139Z [1m[92m  Downloaded[0m piston-float v1.0.1
2026-02-18T12:20:58.0092863Z [1m[92m  Downloaded[0m maybe-owned v0.3.4
2026-02-18T12:20:58.0106871Z [1m[92m  Downloaded[0m byteorder v1.5.0
2026-02-18T12:20:58.0317146Z [1m[92m  Downloaded[0m utf16string v0.2.0
2026-02-18T12:20:58.0336555Z [1m[92m  Downloaded[0m console_log v1.0.0
2026-02-18T12:20:58.0349858Z [1m[92m  Downloaded[0m wasm-bindgen-futures v0.4.58
2026-02-18T12:20:58.0405831Z [1m[92m  Downloaded[0m bytes v1.11.1
2026-02-18T12:20:58.0555356Z [1m[92m  Downloaded[0m itertools v0.14.0
2026-02-18T12:20:58.0701247Z [1m[92m  Downloaded[0m console_error_panic_hook v0.1.7
2026-02-18T12:20:58.0819913Z [1m[92m  Downloaded[0m web-sys v0.3.85
2026-02-18T12:20:58.2050329Z [1m[92m  Downloaded[0m pdfium-render v0.8.37
2026-02-18T12:20:58.3440628Z [1m[92m   Compiling[0m proc-macro2 v1.0.106
2026-02-18T12:20:58.3441531Z [1m[92m   Compiling[0m unicode-ident v1.0.24
2026-02-18T12:20:58.3442254Z [1m[92m   Compiling[0m quote v1.0.44
2026-02-18T12:20:58.3442665Z [1m[92m   Compiling[0m cfg-if v1.0.4
2026-02-18T12:20:58.3763383Z [1m[92m   Compiling[0m typenum v1.19.0
2026-02-18T12:20:58.4060781Z [1m[92m   Compiling[0m version_check v0.9.5
2026-02-18T12:20:58.5845286Z [1m[92m   Compiling[0m generic-array v0.14.7
2026-02-18T12:20:58.9212336Z [1m[92m   Compiling[0m wasm-bindgen-shared v0.2.108
2026-02-18T12:20:58.9499160Z [1m[92m   Compiling[0m rustversion v1.0.22
2026-02-18T12:20:59.0564622Z [1m[92m   Compiling[0m autocfg v1.5.0
2026-02-18T12:20:59.5424542Z [1m[92m   Compiling[0m syn v2.0.116
2026-02-18T12:20:59.5743494Z [1m[92m   Compiling[0m num-traits v0.2.19
2026-02-18T12:20:59.6612509Z [1m[92m   Compiling[0m memchr v2.8.0
2026-02-18T12:20:59.7295379Z [1m[92m   Compiling[0m bumpalo v3.19.1
2026-02-18T12:20:59.9694827Z [1m[92m   Compiling[0m wasm-bindgen v0.2.108
2026-02-18T12:21:00.0885023Z [1m[92m   Compiling[0m simd-adler32 v0.3.8
2026-02-18T12:21:00.1763963Z [1m[92m   Compiling[0m crypto-common v0.1.7
2026-02-18T12:21:00.2325718Z [1m[92m   Compiling[0m crc32fast v1.5.0
2026-02-18T12:21:00.3788938Z [1m[92m   Compiling[0m crossbeam-utils v0.8.21
2026-02-18T12:21:00.5266473Z [1m[92m   Compiling[0m adler2 v2.0.1
2026-02-18T12:21:00.6184937Z [1m[92m   Compiling[0m futures-core v0.3.32
2026-02-18T12:21:00.6979428Z [1m[92m   Compiling[0m futures-sink v0.3.32
2026-02-18T12:21:00.7210750Z [1m[92m   Compiling[0m once_cell v1.21.3
2026-02-18T12:21:00.7548055Z [1m[92m   Compiling[0m futures-channel v0.3.32
2026-02-18T12:21:00.8944369Z [1m[92m   Compiling[0m miniz_oxide v0.8.9
2026-02-18T12:21:01.1044946Z [1m[92m   Compiling[0m block-padding v0.3.3
2026-02-18T12:21:01.1445347Z [1m[92m   Compiling[0m futures-io v0.3.32
2026-02-18T12:21:01.1749726Z [1m[92m   Compiling[0m zerocopy v0.8.39
2026-02-18T12:21:01.2671239Z [1m[92m   Compiling[0m bitflags v2.11.0
2026-02-18T12:21:01.4047765Z [1m[92m   Compiling[0m dtor-proc-macro v0.0.6
2026-02-18T12:21:01.5606862Z [1m[92m   Compiling[0m getrandom v0.3.4
2026-02-18T12:21:01.5806394Z [1m[92m   Compiling[0m pin-project-lite v0.2.16
2026-02-18T12:21:01.6244925Z [1m[92m   Compiling[0m slab v0.4.12
2026-02-18T12:21:01.7310493Z [1m[92m   Compiling[0m futures-task v0.3.32
2026-02-18T12:21:01.7756953Z [1m[92m   Compiling[0m flate2 v1.1.9
2026-02-18T12:21:01.8384914Z [1m[92m   Compiling[0m inout v0.1.4
2026-02-18T12:21:02.2855288Z [1m[92m   Compiling[0m napi-build v2.3.1
2026-02-18T12:21:02.3965863Z [1m[92m   Compiling[0m crossbeam-epoch v0.9.18
2026-02-18T12:21:02.4293453Z [1m[92m   Compiling[0m cipher v0.4.4
2026-02-18T12:21:02.6687658Z [1m[92m   Compiling[0m pxfm v0.1.27
2026-02-18T12:21:02.7242747Z [1m[92m   Compiling[0m block-buffer v0.10.4
2026-02-18T12:21:02.8033441Z [1m[92m   Compiling[0m fdeflate v0.3.7
2026-02-18T12:21:03.7255607Z [1m[92m   Compiling[0m wasm-bindgen-macro-support v0.2.108
2026-02-18T12:21:04.9083654Z [1m[92m   Compiling[0m futures-macro v0.3.32
2026-02-18T12:21:05.5629215Z [1m[92m   Compiling[0m futures-util v0.3.32
2026-02-18T12:21:06.7024644Z [1m[92m   Compiling[0m wasm-bindgen-macro v0.2.108
2026-02-18T12:21:06.7636501Z [1m[92m   Compiling[0m ctor-proc-macro v0.0.7
2026-02-18T12:21:06.9335082Z [1m[92m   Compiling[0m either v1.15.0
2026-02-18T12:21:07.1044961Z [1m[92m   Compiling[0m bytemuck v1.25.0
2026-02-18T12:21:07.3555923Z [1m[92m   Compiling[0m zune-core v0.5.1
2026-02-18T12:21:07.5722070Z [1m[92m   Compiling[0m tinyvec_macros v0.1.1
2026-02-18T12:21:07.6017008Z [1m[92m   Compiling[0m rayon-core v1.13.0
2026-02-18T12:21:07.6847820Z [1m[92m   Compiling[0m tinyvec v1.10.0
2026-02-18T12:21:08.1064866Z [1m[92m   Compiling[0m js-sys v0.3.85
2026-02-18T12:21:08.1415317Z [1m[92m   Compiling[0m zune-jpeg v0.5.12
2026-02-18T12:21:08.5630378Z [1m[92m   Compiling[0m ppv-lite86 v0.2.21
2026-02-18T12:21:08.7919967Z [1m[92m   Compiling[0m moxcms v0.7.11
2026-02-18T12:21:09.3019827Z [1m[92m   Compiling[0m png v0.18.1
2026-02-18T12:21:12.1440567Z [1m[92m   Compiling[0m digest v0.10.7
2026-02-18T12:21:12.1991602Z [1m[92m   Compiling[0m rand_core v0.9.5
2026-02-18T12:21:12.3023244Z [1m[92m   Compiling[0m crossbeam-deque v0.8.6
2026-02-18T12:21:12.3536248Z [1m[92m   Compiling[0m powerfmt v0.2.0
2026-02-18T12:21:12.4665117Z [1m[92m   Compiling[0m thiserror v2.0.18
2026-02-18T12:21:12.4928805Z [1m[92m   Compiling[0m byteorder-lite v0.1.0
2026-02-18T12:21:12.6511938Z [1m[92m   Compiling[0m jiff-tzdb v0.1.5
2026-02-18T12:21:12.7412814Z [1m[92m   Compiling[0m unicode-segmentation v1.12.0
2026-02-18T12:21:12.7760529Z [1m[92m   Compiling[0m log v0.4.29
2026-02-18T12:21:12.9350058Z [1m[92m   Compiling[0m image v0.25.9
2026-02-18T12:21:13.0759192Z [1m[92m   Compiling[0m convert_case v0.11.0
2026-02-18T12:21:13.3211327Z [1m[92m   Compiling[0m jiff-tzdb-platform v0.1.3
2026-02-18T12:21:13.4062593Z [1m[92m   Compiling[0m deranged v0.5.6
2026-02-18T12:21:15.4974205Z [1m[92m   Compiling[0m rand_chacha v0.9.0
2026-02-18T12:21:15.5689823Z [1m[92m   Compiling[0m web-sys v0.3.85
2026-02-18T12:21:16.3195954Z [1m[92m   Compiling[0m futures-executor v0.3.32
2026-02-18T12:21:16.5273912Z [1m[92m   Compiling[0m unicode-normalization v0.1.25
2026-02-18T12:21:16.9986197Z [1m[92m   Compiling[0m thiserror-impl v2.0.18
2026-02-18T12:21:17.9786037Z [1m[92m   Compiling[0m napi v3.8.3
2026-02-18T12:21:18.0690900Z [1m[92m   Compiling[0m dtor v0.1.1
2026-02-18T12:21:18.1386103Z [1m[92m   Compiling[0m chrono v0.4.43
2026-02-18T12:21:18.1411118Z [1m[92m   Compiling[0m nom v8.0.0
2026-02-18T12:21:20.9007147Z [1m[92m   Compiling[0m hashbrown v0.16.1
2026-02-18T12:21:21.5006943Z [1m[92m   Compiling[0m bytecount v0.6.9
2026-02-18T12:21:21.5575864Z [1m[92m   Compiling[0m itoa v1.0.17
2026-02-18T12:21:21.6097230Z [1m[92m   Compiling[0m unicode-properties v0.1.4
2026-02-18T12:21:21.6979794Z [1m[92m   Compiling[0m byteorder v1.5.0
2026-02-18T12:21:21.7826682Z [1m[92m   Compiling[0m equivalent v1.0.2
2026-02-18T12:21:21.8137452Z [1m[92m   Compiling[0m pdfium-render v0.8.37
2026-02-18T12:21:21.9056725Z [1m[92m   Compiling[0m time-core v0.1.8
2026-02-18T12:21:21.9155414Z [1m[92m   Compiling[0m unicode-bidi v0.3.18
2026-02-18T12:21:22.0185367Z [1m[92m   Compiling[0m num-conv v0.2.0
2026-02-18T12:21:22.0907907Z [1m[92m   Compiling[0m libloading v0.9.0
2026-02-18T12:21:22.1826998Z [1m[92m   Compiling[0m semver v1.0.27
2026-02-18T12:21:22.4235299Z [1m[92m   Compiling[0m piston-float v1.0.1
2026-02-18T12:21:22.4743767Z [1m[92m   Compiling[0m vecmath v1.0.0
2026-02-18T12:21:22.6730212Z [1m[92m   Compiling[0m napi-derive-backend v5.0.2
2026-02-18T12:21:22.7075321Z [1m[92m   Compiling[0m stringprep v0.1.5
2026-02-18T12:21:23.8613702Z [1m[92m   Compiling[0m napi-sys v3.2.1
2026-02-18T12:21:23.9221591Z [1m[92m   Compiling[0m time v0.3.47
2026-02-18T12:21:25.1664853Z [1m[92m   Compiling[0m utf16string v0.2.0
2026-02-18T12:21:25.3665110Z [1m[92m   Compiling[0m indexmap v2.13.0
2026-02-18T12:21:26.2008491Z [1m[92m   Compiling[0m nom_locate v5.0.0
2026-02-18T12:21:26.3287553Z [1m[92m   Compiling[0m ctor v0.6.3
2026-02-18T12:21:26.4064884Z [1m[92m   Compiling[0m console_log v1.0.0
2026-02-18T12:21:26.4653872Z [1m[92m   Compiling[0m futures v0.3.32
2026-02-18T12:21:26.4825261Z [1m[92m   Compiling[0m rand v0.9.2
2026-02-18T12:21:26.5001882Z [1m[92m   Compiling[0m rayon v1.11.0
2026-02-18T12:21:27.0896635Z [1m[92m   Compiling[0m jiff v0.2.20
2026-02-18T12:21:28.2836173Z [1m[92m   Compiling[0m sha2 v0.10.9
2026-02-18T12:21:28.7304921Z [1m[92m   Compiling[0m md-5 v0.10.6
2026-02-18T12:21:28.8205272Z [1m[92m   Compiling[0m wasm-bindgen-futures v0.4.58
2026-02-18T12:21:28.9755113Z [1m[92m   Compiling[0m console_error_panic_hook v0.1.7
2026-02-18T12:21:29.0641919Z [1m[92m   Compiling[0m fearless_simd v0.3.0
2026-02-18T12:21:29.0887586Z [1m[92m   Compiling[0m itertools v0.14.0
2026-02-18T12:21:30.7485450Z [1m[92m   Compiling[0m ecb v0.1.2
2026-02-18T12:21:30.7998099Z [1m[92m   Compiling[0m cbc v0.1.2
2026-02-18T12:21:30.8793475Z [1m[92m   Compiling[0m aes v0.8.4
2026-02-18T12:21:31.1409578Z [1m[92m   Compiling[0m pdfdown v0.9.8 (/home/runner/work/pdfdown/pdfdown)
2026-02-18T12:21:31.2277870Z [1m[92m   Compiling[0m encoding_rs v0.8.35
2026-02-18T12:21:31.2964910Z [1m[92m   Compiling[0m rustc-hash v2.1.1
2026-02-18T12:21:31.3488467Z [1m[92m   Compiling[0m bytes v1.11.1
2026-02-18T12:21:32.2155118Z [1m[92m   Compiling[0m nohash-hasher v0.2.0
2026-02-18T12:21:32.2598409Z [1m[92m   Compiling[0m weezl v0.1.12
2026-02-18T12:21:32.8249077Z [1m[92m   Compiling[0m rangemap v1.7.1
2026-02-18T12:21:33.1256814Z [1m[92m   Compiling[0m ttf-parser v0.25.1
2026-02-18T12:21:33.7654650Z [1m[92m   Compiling[0m maybe-owned v0.3.4
2026-02-18T12:21:34.7508569Z [1m[92m   Compiling[0m hayro-jpeg2000 v0.3.2
2026-02-18T12:21:37.1285326Z [1m[92m   Compiling[0m lopdf v0.39.0
2026-02-18T12:21:38.5177943Z [1m[92m   Compiling[0m napi-derive v3.5.2
2026-02-18T12:21:48.5759031Z [1m[91merror[E0599][0m[1m: no function or associated item named `bind_to_library` found for struct `pdfium_render::prelude::Pdfium` in the current scope[0m
2026-02-18T12:21:48.5760061Z    [1m[94m--> [0msrc/core/render.rs:17:11
2026-02-18T12:21:48.5760556Z     [1m[94m|[0m
2026-02-18T12:21:48.5761375Z  [1m[94m17[0m [1m[94m|[0m   Pdfium::bind_to_library(path).ok().map(Pdfium::new)
2026-02-18T12:21:48.5762573Z     [1m[94m|[0m           [1m[91m^^^^^^^^^^^^^^^[0m [1m[91mfunction or associated item not found in `pdfium_render::prelude::Pdfium`[0m
2026-02-18T12:21:48.5763537Z     [1m[94m|[0m
2026-02-18T12:21:48.5764696Z [1m[92mnote[0m: if you're trying to build a new `pdfium_render::prelude::Pdfium`, consider using `pdfium_render::prelude::Pdfium::new` which returns `pdfium_render::prelude::Pdfium`
2026-02-18T12:21:48.5766037Z    [1m[94m--> [0m/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pdfium-render-0.8.37/src/pdfium.rs:151:5
2026-02-18T12:21:48.5766684Z     [1m[94m|[0m
2026-02-18T12:21:48.5767278Z [1m[94m151[0m [1m[94m|[0m     pub fn new(bindings: Box<dyn PdfiumLibraryBindings>) -> Self {
2026-02-18T12:21:48.5768222Z     [1m[94m|[0m     [1m[92m^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^[0m
2026-02-18T12:21:48.5768991Z [1m[96mhelp[0m: there is an associated function `bind_to_system_library` with a similar name
2026-02-18T12:21:48.5769984Z    [1m[94m--> [0m/home/runner/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/pdfium-render-0.8.37/src/pdfium.rs:97:5
2026-02-18T12:21:48.5770624Z     [1m[94m|[0m
2026-02-18T12:21:48.5771283Z  [1m[94m97[0m [1m[94m|[0m     pub fn bind_to_system_library() -> Result<Box<dyn PdfiumLibraryBindings>, PdfiumError> {
2026-02-18T12:21:48.5772446Z     [1m[94m|[0m     [1m[96m^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^[0m
2026-02-18T12:21:48.5772800Z 
2026-02-18T12:21:48.8647295Z [1mFor more information about this error, try `rustc --explain E0599`.[0m
2026-02-18T12:21:48.8710908Z [1m[91merror[0m: could not compile `pdfdown` (lib) due to 1 previous error
2026-02-18T12:21:48.8879367Z [31m[1mInternal Error[22m[39m: Build failed with exit code 101
2026-02-18T12:21:48.8880145Z     at ChildProcess.<anonymous> (file:///home/runner/work/pdfdown/pdfdown/node_modules/@napi-rs/cli/dist/cli.js:1604:36)
2026-02-18T12:21:48.8880768Z     at Object.onceWrapper (node:events:623:26)
2026-02-18T12:21:48.8881428Z     at ChildProcess.emit (node:events:520:35)
2026-02-18T12:21:48.8881880Z     at ChildProcess._handle.onexit (node:internal/child_process:294:12)
2026-02-18T12:21:48.9189794Z ##[error]Process completed with exit code 1.
2026-02-18T12:21:48.9278899Z Post job cleanup.
2026-02-18T12:21:49.0083174Z [command]/usr/bin/git version
2026-02-18T12:21:49.0120289Z git version 2.52.0
2026-02-18T12:21:49.0192524Z Temporarily overriding HOME='/home/runner/work/_temp/d36a3e90-e04c-4578-be7f-73d359353149' before making global git config changes
2026-02-18T12:21:49.0194314Z Adding repository directory to the temporary git global config as a safe directory
2026-02-18T12:21:49.0199010Z [command]/usr/bin/git config --global --add safe.directory /home/runner/work/pdfdown/pdfdown
2026-02-18T12:21:49.0231157Z Removing SSH command configuration
2026-02-18T12:21:49.0237917Z [command]/usr/bin/git config --local --name-only --get-regexp core\.sshCommand
2026-02-18T12:21:49.0278045Z [command]/usr/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'core\.sshCommand' && git config --local --unset-all 'core.sshCommand' || :"
2026-02-18T12:21:49.0521699Z Removing HTTP extra header
2026-02-18T12:21:49.0527350Z [command]/usr/bin/git config --local --name-only --get-regexp http\.https\:\/\/github\.com\/\.extraheader
2026-02-18T12:21:49.0561478Z [command]/usr/bin/git submodule foreach --recursive sh -c "git config --local --name-only --get-regexp 'http\.https\:\/\/github\.com\/\.extraheader' && git config --local --unset-all 'http.https://github.com/.extraheader' || :"
2026-02-18T12:21:49.0795558Z Removing includeIf entries pointing to credentials config files
2026-02-18T12:21:49.0802280Z [command]/usr/bin/git config --local --name-only --get-regexp ^includeIf\.gitdir:
2026-02-18T12:21:49.0827060Z includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git.path
2026-02-18T12:21:49.0828063Z includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git/worktrees/*.path
2026-02-18T12:21:49.0828584Z includeif.gitdir:/github/workspace/.git.path
2026-02-18T12:21:49.0829357Z includeif.gitdir:/github/workspace/.git/worktrees/*.path
2026-02-18T12:21:49.0837651Z [command]/usr/bin/git config --local --get-all includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git.path
2026-02-18T12:21:49.0860532Z /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.0871462Z [command]/usr/bin/git config --local --unset includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git.path /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.0907770Z [command]/usr/bin/git config --local --get-all includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git/worktrees/*.path
2026-02-18T12:21:49.0930910Z /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.0941324Z [command]/usr/bin/git config --local --unset includeif.gitdir:/home/runner/work/pdfdown/pdfdown/.git/worktrees/*.path /home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.0974031Z [command]/usr/bin/git config --local --get-all includeif.gitdir:/github/workspace/.git.path
2026-02-18T12:21:49.0997167Z /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.1005881Z [command]/usr/bin/git config --local --unset includeif.gitdir:/github/workspace/.git.path /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.1039295Z [command]/usr/bin/git config --local --get-all includeif.gitdir:/github/workspace/.git/worktrees/*.path
2026-02-18T12:21:49.1061766Z /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.1072708Z [command]/usr/bin/git config --local --unset includeif.gitdir:/github/workspace/.git/worktrees/*.path /github/runner_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config
2026-02-18T12:21:49.1106518Z [command]/usr/bin/git submodule foreach --recursive git config --local --show-origin --name-only --get-regexp remote.origin.url
2026-02-18T12:21:49.1348679Z Removing credentials config '/home/runner/work/_temp/git-credentials-cda7a7a3-ea6a-4a6c-bd2b-2a7c7d0be717.config'
2026-02-18T12:21:49.1476632Z Cleaning up orphan processes
```
