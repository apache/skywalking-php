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


use PhpAmqpLib\Connection\AMQPStreamConnection;
use PhpAmqpLib\Message\AMQPMessage;
use PhpAmqpLib\Wire\AMQPTable;


require_once dirname(__DIR__) . "/vendor/autoload.php";

$connection = new AMQPStreamConnection("127.0.0.1", 5672, 'guest', 'guest');
$channel = $connection->channel();

$channel->queue_declare('queue_test', false, false, false, false);
$channel->exchange_declare('exchange_test', 'direct', false, false, false);
$channel->queue_bind('queue_test', 'exchange_test', 'routing_test');

{
    $msg = new AMQPMessage('Hello World!');
    $channel->basic_publish($msg, '', 'queue_test');
}

{
    $msg = new AMQPMessage('Hello World!', ['content_type' => 'text/plain']);
    $channel->basic_publish($msg, 'exchange_test', 'routing_test');
}

{
    $msg = new AMQPMessage('Hello World!');
    $msg->set('application_headers', new AMQPTable(['foo' => 'bar']));
    $channel->basic_publish($msg, '', 'not_exists');
}

echo "ok";
