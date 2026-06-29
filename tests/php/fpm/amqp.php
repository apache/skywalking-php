<?php

// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

function assertTrue(bool $condition, string $message): void
{
    if (!$condition) {
        throw new RuntimeException($message);
    }
}

function assertSameValue($expected, $actual, string $message): void
{
    if ($expected !== $actual) {
        throw new RuntimeException(sprintf('%s. expected=%s actual=%s', $message, var_export($expected, true), var_export($actual, true)));
    }
}

function expectEnvelope(AMQPQueue $queue, string $body, callable $assertions): void
{
    $envelope = null;
    for ($attempt = 0; $attempt < 10; $attempt++) {
        $envelope = $queue->get(AMQP_AUTOACK);
        if ($envelope instanceof AMQPEnvelope) {
            break;
        }
        usleep(100000);
    }

    assertTrue($envelope instanceof AMQPEnvelope, 'expected message in queue');
    assertSameValue($body, $envelope->getBody(), 'unexpected message body');
    $assertions($envelope);
}


$connection = new AMQPConnection(['host' => '127.0.0.1', 'port' => 5672, 'login' => 'guest', 'password' => 'guest']);
$connection->connect();
$channel = new AMQPChannel($connection);

$queue = new AMQPQueue($channel);
$queue->setName('queue_test');
$queue->setFlags(AMQP_NOPARAM);
$queue->declareQueue();

$exchange = new AMQPExchange($channel);
$exchange->setName('exchange_test');
$exchange->setType(AMQP_EX_TYPE_DIRECT);
$exchange->declareExchange();

$fanoutQueue = new AMQPQueue($channel);
$fanoutQueue->setName('fanout_queue_test');
$fanoutQueue->setFlags(AMQP_NOPARAM);
$fanoutQueue->declareQueue();

$fanoutExchange = new AMQPExchange($channel);
$fanoutExchange->setName('fanout_exchange_test');
$fanoutExchange->setType(AMQP_EX_TYPE_FANOUT);
$fanoutExchange->declareExchange();

$queue->bind('exchange_test', 'routing_test');
$fanoutQueue->bind('fanout_exchange_test');

$queue->purge();
$fanoutQueue->purge();

{
    $exchange = new AMQPExchange($channel);
    $exchange->setName('fanout_exchange_test');
    $exchange->publish('One Arg Message');

    expectEnvelope($fanoutQueue, 'One Arg Message', static function (AMQPEnvelope $envelope): void {
    });
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->publish('Two Arg Message', 'queue_test');

    expectEnvelope($queue, 'Two Arg Message', static function (AMQPEnvelope $envelope): void {
    });
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->setName('exchange_test');
    $exchange->publish('Three Arg Message', 'routing_test', AMQP_NOPARAM);

    expectEnvelope($queue, 'Three Arg Message', static function (AMQPEnvelope $envelope): void {
    });
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->publish('Four Arg Message', 'queue_test', AMQP_NOPARAM, ['headers' => ['foo' => 'bar']]);

    expectEnvelope($queue, 'Four Arg Message', static function (AMQPEnvelope $envelope): void {
        assertTrue($envelope->hasHeader('foo'), 'missing custom header for four-arg publish');
        assertSameValue('bar', $envelope->getHeader('foo'), 'custom header should be preserved');
    });
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->publish('Default Exchange Message', 'queue_test', AMQP_NOPARAM);

    expectEnvelope($queue, 'Default Exchange Message', static function (AMQPEnvelope $envelope): void {
    });
}

echo "ok";
