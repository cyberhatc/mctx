# FreeBSD port skeleton for mctx.
#
# To install with the pkg toolchain:
#   1. place this directory at ports-tree/binary/mctx (or use `make` in place)
#   2. cd into it and run `make makesum` to record the crates distfiles,
#   3. `make package`  -> produces mctx-1.1.0.pkg,
#   4. `pkg add ./mctx-1.1.0.pkg`  (or publish it to a local pkg repo and
#      `pkg install mctx`).
#
# Submit this to FreeBSD ports as `binary/mctx` to get it into the official
# `pkg install mctx` index.

PORTNAME=	mctx
PORTVERSION=	1.1.0
CATEGORIES=	binary textproc
DISTVERSIONPREFIX=	v

MAINTAINER=	cyberhatc@users.noreply.github.com
WWW=		https://github.com/cyberhatc/mctx
COMMENT=	Terminal notepad for .mctx AI agent memory files
LICENSE=	MIT APACHE20

USES=		cargo
USE_GITHUB=	yes
GH_ACCOUNT=	cyberhatc
GH_PROJECT=	mctx

CARGO_CRATES=	you-fill-via-make-makesum

PLIST_FILES=	bin/mctx \
		man/man1/mctx.1.gz

post-install:
	${INSTALL_PROGRAM} ${WRKSRC}/target/release/mctx ${STAGEDIR}${PREFIX}/bin/mctx
	${INSTALL_MAN} ${WRKSRC}/man/mctx.1 ${STAGEDIR}${MANPREFIX}/man/man1/mctx.1

.include <bsd.port.mk>
