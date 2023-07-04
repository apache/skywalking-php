# Zend observer

> Refer to: <https://www.datadoghq.com/blog/engineering/php-8-observability-baked-right-in/#the-observability-landscape-before-php-8>

By default, skywalking-php hooks the `zend_execute_internal` and `zend_execute_ex` functions to implement auto instrumentation.

But there are some drawbacks:

- All PHP function calls are placed on the native C stack, which is limited by the value set in `ulimit -s`.
- Not compatible with the new JIT added in PHP 8.

## The observer API in PHP 8+

Now, zend observer api is a new generation method, and it is also a method currently recommended by PHP8.

This method has no stack problem and will not affect JIT.

## Configuration

The following configuration example enables JIT in PHP8 and zend observer support in skywalking-php at the same time.

```ini
[opcache]
zend_extension = opcache
; Enable JIT
opcache.jit = tracing

[skywalking_agent]
extension = skywalking_agent.so
; Switch to use zend observer api to implement auto instrumentation.
skywalking_agent.enable_zend_observer = On
```
