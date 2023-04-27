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

use Webmozart\Assert\Assert;

require_once dirname(__DIR__) . "/vendor/autoload.php";

extension_loaded('swoole') or die("extension swoole not loaded");

$http = new Swoole\Http\Server('127.0.0.1', 9502);

$http->set([
    'reactor_num' => 3,
    'worker_num' => 3,
    'enable_coroutine' => true,
    'hook_flags' => SWOOLE_HOOK_ALL,
]);

$http->on('start', function ($server) {
    echo "Swoole http server is started at http://127.0.0.1:9502\n";
});

$http->on('request', function ($request, $response) {
    try {
        switch ($request->server['request_uri']) {
        case "/":
            break;
        
        case '/curl':
            {
                $ch = curl_init();
                curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9502/?swoole=2");
                curl_setopt($ch, CURLOPT_TIMEOUT, 10);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_HEADER, 0);
                $output = curl_exec($ch);
                curl_close($ch);
                Assert::same($output, "ok");
            }
            break;
        
        case '/pdo':
            {
                $pdo = new PDO("mysql:dbname=skywalking;host=127.0.0.1;port=3306", "root", "password");
                $result = $pdo->exec("SELECT 1");
                Assert::notFalse($result);
            }
            break;

        case '/mysqli':
            {
                $mysqli = new mysqli("127.0.0.1", "root", "password", "skywalking", 3306);
                $result = $mysqli->query("SELECT 1");
                Assert::notFalse($result);
            }
            break;

        case '/memcached':
            {
                $mc = new Memcached();
                $mc->addServer("127.0.0.1", 11211);

                $mc->set("foo000", "bar000");
                Assert::same($mc->get("foo000"), 'bar000');
            }
            break;

        case '/redis':
            {
                $client = new Redis();
                $client->connect("127.0.0.1", 6379);
                $client->auth('password');
                $client->set('foo001', 'bar001');
                Assert::same($client->get('foo001'), 'bar001');
            }
            break;
        
        case '/predis':
            {
                $client = new Predis\Client();
                $client->auth('password');
                $client->set('foo002', 'bar002');
                Assert::same($client->get('foo002'), 'bar002');
            }
            break;

        default:
            throw new DomainException("Unknown operation");
        }

        $response->header('Content-Type', 'text/plain');
        $response->end('ok');

    } catch (Exception $e) {
        $response->status(500);
        $response->header('Content-Type', 'text/plain');
        $response->end($e->getMessage() . "\n" . $e->getTraceAsString());
    }
});

$http->start();
