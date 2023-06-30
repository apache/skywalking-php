# Apache SkyWalking PHP Agent

<img src="http://skywalking.apache.org/assets/logo.svg" alt="Sky Walking logo" height="90px" align="right" />

**SkyWalking PHP** The PHP Agent for Apache SkyWalking, which provides the native tracing abilities for PHP project.

**SkyWalking** an APM(application performance monitor) system, especially designed for
microservices, cloud native and container-based (Docker, Kubernetes, Mesos) architectures.

[![GitHub stars](https://img.shields.io/github/stars/apache/skywalking-php.svg?style=for-the-badge&label=Stars&logo=github)](https://github.com/apache/skywalking-php)
[![Twitter Follow](https://img.shields.io/twitter/follow/asfskywalking.svg?style=for-the-badge&label=Follow&logo=twitter)](https://twitter.com/AsfSkyWalking)

## Documentation

* [Official documentation](https://skywalking.apache.org/docs/#PHPAgent)

## How to create issue?

Submit an [GitHub Issue](https://github.com/apache/skywalking/issues/new/choose) in the Apache Skywalking repository, by using [PHP] as title prefix.

## Installation Requirements

SkyWalking PHP Agent requires SkyWalking 8.4+ and PHP 7.2+

## Support List

* PHP-FPM Ecosystem
  * [x] [cURL](https://www.php.net/manual/en/book.curl.php#book.curl)
  * [x] [PDO](https://www.php.net/manual/en/book.pdo.php)
  * [x] [MySQL Improved](https://www.php.net/manual/en/book.mysqli.php)
  * [x] [Memcached](https://www.php.net/manual/en/book.memcached.php)
  * [x] [phpredis](https://github.com/phpredis/phpredis)
  * [ ] [php-amqp](https://github.com/php-amqp/php-amqp)
  * [ ] [php-rdkafka](https://github.com/arnaud-lb/php-rdkafka)
  * [x] [predis](https://github.com/predis/predis)
  * [x] [php-amqplib](https://github.com/php-amqplib/php-amqplib) for Message Queuing Producer

* Swoole Ecosystem

  *The components of the PHP-FPM ecosystem can also be used in Swoole regardless of the flag `SWOOLE_HOOK_ALL`.*

## Contact Us

* Mail list: **dev@skywalking.apache.org**. Mail to `dev-subscribe@skywalking.apache.org`, follow the reply to subscribe the mail list.
* Join `skywalking` channel at [Apache Slack](http://s.apache.org/slack-invite). If the link is not working, find the latest one at [Apache INFRA WIKI](https://cwiki.apache.org/confluence/display/INFRA/Slack+Guest+Invites).
* Twitter, [ASFSkyWalking](https://twitter.com/AsfSkyWalking)

## Stargazers over time

[![Stargazers over time](https://starchart.cc/apache/skywalking-php.svg)](https://starchart.cc/apache/skywalking-php)

## License

[Apache 2.0](LICENSE)
