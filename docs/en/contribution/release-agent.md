# Apache SkyWalking PHP Agent release guide

If you're a committer, you can learn how to release SkyWalking SDK in The Apache Way and start the voting process by
reading this document.

## Requirements

- Rust(rustc)
- Cargo
- PHP(php, php-config)
- Pecl
- GPG
- shasum

## Add your GPG public key

1. Add your GPG public key into the [SkyWalking GPG KEYS](https://dist.apache.org/repos/dist/release/skywalking/KEYS)
   file. If you are a committer, use your Apache ID and password to log in this svn, and update the file. **Don't
   override the existing file.**(Notice, only PMC member could update this file)
2. Upload your GPG public key to the public GPG site, such as [MIT's site](http://pgp.mit.edu:11371/). This site should
   be in the Apache maven staging repository checklist.

## Draft a new release

Open [Create a new release](https://github.com/apache/skywalking-php/releases/new) page, choose the tag, and click the `Generate release notes` button, then copy the generated text to local `/tmp/notes.txt`.

## Test your settings and package

```shell
## Make sure local compiling passed
> cargo build

## Create package.xml from package.xml.tpl
> cargo run -p scripts --release -- create-package-xml --version x.y.z --notes "`cat /tmp/notes.txt`"

## Create local package. The skywalking_agent-x.y.z.tgz should be found in project root
> pecl package
```

## Sign the package

Tag the commit ID of this release as v`x.y.z`.

After set the version in `Cargo.toml` with the release number, package locally. Then run the following commands to sign
your package.

```shell
> export RELEASE_VERSION=x.y.z

## The package should be signed by your Apache committer mail.
> gpg --armor --detach-sig skywalking_agent-$RELEASE_VERSION.tgz

> shasum -a 512 skywalking_agent-$RELEASE_VERSION.tgz > skywalking_agent-$RELEASE_VERSION.tgz.sha512
```

After these, the source tar with its signed asc and sha512 are ready.

## Upload to Apache SVN and tag a release

1. Use your Apache ID to log in to `https://dist.apache.org/repos/dist/dev/skywalking/php`.
2. Create a folder and name it by the release version and round, such as: `x.y.z`
3. Upload tar ball, asc, sha512 files to the new folder.

## Call a vote in dev

Call a vote in `dev@skywalking.apache.org`

```
Mail title: [VOTE] Release Apache SkyWalking PHP version x.y.z

Mail content:
Hi All,
This is a call for vote to release Apache SkyWalking PHP version x.y.z.

Release Candidate:

* https://dist.apache.org/repos/dist/dev/skywalking/php/x.y.z/
* sha512 checksums
- xxxxxxxx skywalking_agent-x.y.z.tgz

Release Tag :

* (Git Tag) vx.y.z

Release CommitID :

* https://github.com/apache/skywalking-php/tree/{commit-id}

Keys to verify the Release Candidate :

* https://dist.apache.org/repos/dist/release/skywalking/KEYS

Guide to build the release from source :

* https://github.com/apache/skywalking-php/blob/master/docs/en/contribution/compiling.md

Voting will start now (Date) and will remain open for at least 72
hours, Request all PMC members to give their vote.
[ ] +1 Release this package.
[ ] +0 No opinion.
[ ] -1 Do not release this package because....
```

## Vote Check

The voting process is as follows:

1. All PMC member votes are +1 binding, and all other votes are +1 but non-binding.
1. If you obtain at least 3 (+1 binding) votes with more +1 than -1 votes within 72 hours, the release will be approved.

## Publish the release

1. Move source codes tar and distribution packages to `https://dist.apache.org/repos/dist/release/skywalking/`.

   ```shell
   > export SVN_EDITOR=vim
   > svn mv https://dist.apache.org/repos/dist/dev/skywalking/php/x.y.z https://dist.apache.org/repos/dist/release/skywalking/php
   ....
   enter your apache password
   ....
   ```

2. Pecl publish package on [skywalking_agent](https://pecl.php.net/package/skywalking_agent).

   Make sure you have a PECL account, and list in `package.tpl.xml` as `<developer>`,
   or reach `private@skywalking.apache.org` if you are a committer/PMC but not listed.

   You can request a PECL account via <https://pecl.php.net/account-request.php>.

3. Add an release event, update download and doc releases on the SkyWalking website.

4. Add the new release on [ASF addrelease site](https://reporter.apache.org/addrelease.html?skywalking).

5. Remove the old releases on `https://dist.apache.org/repos/dist/release/skywalking/php/{previous-version}`.

## Send a release announcement

Send ANNOUNCE email to `dev@skywalking.apache.org`, `announce@apache.org`. The sender should use the Apache email
account.

```txt
Mail title: [ANNOUNCE] Apache SkyWalking PHP x.y.z released

Mail content:
Hi all,

SkyWalking PHP Agent provides the native tracing abilities for PHP project.

SkyWalking: APM (application performance monitor) tool for distributed systems,
especially designed for microservices, cloud native and container-based (Docker, Kubernetes, Mesos) architectures.

This release contains a number of new features, bug fixes and improvements compared to
version a.b.c(last release). The notable changes since x.y.z include:

(Highlight key changes)
1. ...
2. ...
3. ...

Apache SkyWalking website:
http://skywalking.apache.org/

Downloads:
http://skywalking.apache.org/downloads/

Twitter:
https://twitter.com/ASFSkyWalking

SkyWalking Resources:
- GitHub: https://github.com/apache/skywalking
- Issue: https://github.com/apache/skywalking/issues
- Mailing list: dev@skywalkiing.apache.org


- Apache SkyWalking Team
```
