PHP_ARG_ENABLE([skywalking_agent],
  [whether to enable skywalking_agent support],
  [AS_HELP_STRING([--enable-skywalking_agent],
    [Enable skywalking_agent support])],
  [no])

if test "$PHP_THREAD_SAFETY" == "yes"; then
  AC_MSG_ERROR([skywalking_agent does not support ZTS])
fi

if test "$PHP_SKYWALKING_AGENT" != "no"; then
  AC_PATH_PROG(CARGO, cargo, no)
  if ! test -x "$CARGO"; then
    AC_MSG_ERROR([cargo command missing, please reinstall the cargo distribution])
  fi

  AC_PATH_PROG(RUSTFMT, rustfmt, no)
  if ! test -x "$RUSTFMT"; then
    AC_MSG_ERROR([rustfmt command missing, please reinstall the rustfmt distribution])
  fi

  AC_PATH_PROG(PROTOC, protoc, no)
  if ! test -x "$PROTOC"; then
    AC_MSG_ERROR([protoc command missing, please reinstall the protoc distribution])
  fi

  AC_DEFINE(HAVE_SKYWALKING_AGENT, 1, [ Have skywalking_agent support ])

  PHP_NEW_EXTENSION(skywalking_agent, Cargo.toml, $ext_shared)
fi
