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


$connection = new AMQPConnection(['host' => '127.0.0.1', 'port' => 5672, 'login' => 'guest', 'password' => 'guest']);
$connection->connect();
$channel = new AMQPChannel($connection);

$channel->queueDeclare('queue_test');
$channel->exchangeDeclare('exchange_test', AMQP_EX_TYPE_DIRECT);
$channel->queueBind('queue_test', 'exchange_test', 'routing_test');

{
    $exchange = new AMQPExchange($channel);
    $exchange->setName('');
    $exchange->publish('Hello World!', 'queue_test', AMQP_NOPARAM, []);
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->setName('exchange_test');
    $exchange->publish('Hello World!', 'routing_test', AMQP_NOPARAM, []);
}

{
    $exchange = new AMQPExchange($channel);
    $exchange->setName('');
    $exchange->publish('Hello World!', 'not_exists', AMQP_NOPARAM, ['foo' => 'bar']);
}

echo "ok";
