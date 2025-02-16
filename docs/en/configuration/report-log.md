# Report Log

## Overview

In `skywalking-php`, the log level configuration is managed using the `skywalking_agent.psr_logging_level` directive in your `php.ini` file. This configuration defines the minimum log level that will be reported to SkyWalking. The log levels are based on PSR-3 standards and allow you to control the verbosity of the logs sent to SkyWalking.

## Configuration

You can set the `skywalking_agent.psr_logging_level` in your `php.ini` file:

```ini
skywalking_agent.psr_logging_level = Info
```

The possible values for this configuration are:

- `Off`: No logs will be reported to SkyWalking.
- `Debug`: Logs with the level "Debug" or higher will be reported.
- `Info`: Logs with the level "Info" or higher will be reported.
- `Notice`: Logs with the level "Notice" or higher will be reported.
- `Warning`: Logs with the level "Warning" or higher will be reported.
- `Error`: Logs with the level "Error" or higher will be reported.
- `Critical`: Logs with the level "Critical" or higher will be reported.
- `Alert`: Logs with the level "Alert" or higher will be reported.
- `Emergency`: Logs with the level "Emergency" or higher will be reported.

### Default Value

The default value for `skywalking_agent.psr_logging_level` is set to `Off`, which means no log will be reported to SkyWalking unless specified otherwise.

## How It Works

The `skywalking_agent.psr_logging_level` setting works by hooking into any PHP `LoggerInterface` implementation that follows the PSR-3 standard. The agent listens for log events and compares the log level with the configured value.

- If the log level is **greater than or equal to** the specified `skywalking_agent.psr_logging_level`, the log is reported to SkyWalking.
- Logs with a level **lower than** the configured value will be ignored and not sent to SkyWalking.

This approach ensures that only relevant logs (those that meet or exceed the configured severity level) are sent to SkyWalking, minimizing noise and focusing on more critical events.

## Example Usage

To report logs of level `Warning` and higher to SkyWalking, you would set the configuration as follows:

```ini
skywalking_agent.psr_logging_level = Warning
```

With this setting, logs at the levels `Warning`, `Error`, `Critical`, `Alert`, and `Emergency` will be sent to SkyWalking, while logs at the `Debug`, `Info`, and `Notice` levels will be ignored.

## Conclusion

The `skywalking_agent.psr_logging_level` configuration gives you fine-grained control over the logging behavior of your SkyWalking PHP agent. Adjusting the log level allows you to ensure that only the most important logs are captured, optimizing your monitoring and debugging workflows.
