# Context injection

If you want to fetch the SkyWalking Context in your PHP code, which is super helpful for debugging and observability,
You can enable the configuration item `skywalking_agent.inject_context`.

## Description

`skywalking_agent.inject_context`

Whether to enable automatic injection of skywalking context variables (such as `SW_TRACE_ID`). For `php-fpm` mode, it will be injected into the `$_SERVER` variable. For `swoole` mode, it will be injected into the `$request->server` variable.

## Configuration

```ini
[skywalking_agent]
extension = skywalking_agent.so
skywalking_agent.inject_context = On
```

## Usage

For `php-fpm` mode:

```php
<?php

echo $_SERVER["SW_SERVICE_NAME"]; // get service name
echo $_SERVER["SW_INSTANCE_NAME"]; // get instance name
echo $_SERVER["SW_TRACE_ID"]; // get trace id
echo $_SERVER["SW_TRACE_SEGMENT_ID"]; // get trace segment id
```

For `swoole` mode:

```php
<?php

$http = new Swoole\Http\Server('127.0.0.1', 9501);

$http->on('request', function ($request, $response) {
    echo $request->server["SW_SERVICE_NAME"]; // get service name
    echo $request->server["SW_INSTANCE_NAME"]; // get instance name
    echo $request->server["SW_TRACE_ID"]; // get trace id
    echo $request->server["SW_TRACE_SEGMENT_ID"]; // get trace segment id
});
```
