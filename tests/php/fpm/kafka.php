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

$conf = new RdKafka\Conf();
$conf->set('bootstrap.servers', '127.0.0.1:9092');
$conf->set('group.id', 'sw_test_' . uniqid());
$conf->set('auto.offset.reset', 'earliest');
$conf->set('enable.auto.commit', 'true');
$consumer = new RdKafka\KafkaConsumer($conf);
$consumer->subscribe(['test_topic']);

// Drain leftover messages from previous runs.
while (true) {
    $msg = $consumer->consume(1000);
    if ($msg === null || $msg->err === RD_KAFKA_RESP_ERR__TIMED_OUT) {
        break;
    }
}

// Produce messages.
$conf = new RdKafka\Conf();
$conf->set('bootstrap.servers', '127.0.0.1:9092');
$producer = new RdKafka\Producer($conf);
$producer->addBrokers('127.0.0.1:9092');

$topic = $producer->newTopic('test_topic');

$topic->producev(RD_KAFKA_PARTITION_UA, 0);
$topic->producev(RD_KAFKA_PARTITION_UA, 0, 'Message With Payload');
$topic->producev(RD_KAFKA_PARTITION_UA, 0, 'Message With Key', 'my_key');
$topic->producev(RD_KAFKA_PARTITION_UA, 0, 'Message With Headers', 'my_key', ['foo' => 'bar']);
$topic->producev(RD_KAFKA_PARTITION_UA, 0, 'Full Message', 'my_key', ['foo' => 'bar'], 1234567890);

$producer->flush(10000);

// Consume and verify new messages.
$received = 0;
$maxMessages = 5;
$maxAttempts = 30;

for ($attempt = 0; $attempt < $maxAttempts && $received < $maxMessages; $attempt++) {
    $msg = $consumer->consume(2000);

    if ($msg === null) {
        continue;
    }

    if ($msg->err === RD_KAFKA_RESP_ERR__PARTITION_EOF) {
        continue;
    }

    if ($msg->err === RD_KAFKA_RESP_ERR__TIMED_OUT) {
        continue;
    }

    if ($msg->err !== RD_KAFKA_RESP_ERR_NO_ERROR) {
        throw new RuntimeException(sprintf('consume error: %s', $msg->errstr()));
    }

    $msgIndex = $received + 1;
    if ($msgIndex === 1) {
        assertSameValue(null, $msg->payload, 'message 1 should have no payload');
    } elseif ($msgIndex === 2) {
        assertSameValue('Message With Payload', $msg->payload, 'message 2 payload mismatch');
    } elseif ($msgIndex === 3) {
        assertSameValue('Message With Key', $msg->payload, 'message 3 payload mismatch');
    } elseif ($msgIndex === 4) {
        assertSameValue('Message With Headers', $msg->payload, 'message 4 payload mismatch');
        assertTrue(isset($msg->headers['foo']), 'message 4 missing header foo');
        assertSameValue('bar', $msg->headers['foo'], 'message 4 header foo value mismatch');
    } elseif ($msgIndex === 5) {
        assertSameValue('Full Message', $msg->payload, 'message 5 payload mismatch');
        assertTrue(isset($msg->headers['foo']), 'message 5 missing header foo');
        assertSameValue('bar', $msg->headers['foo'], 'message 5 header foo value mismatch');
    }

    $received++;
}

assertTrue($received === $maxMessages, sprintf('expected %d messages, got %d', $maxMessages, $received));

echo "ok";
