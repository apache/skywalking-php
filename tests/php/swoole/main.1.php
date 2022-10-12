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

$http = new Swoole\Http\Server('127.0.0.1', 9501);

$http->set([
    'reactor_num' => 3,
    'worker_num' => 3,
    'enable_coroutine' => true,
    'hook_flags' => 0,
]);

$http->on('start', function ($server) {
    echo "Swoole http server is started at http://127.0.0.1:9501\n";
});

$http->on('request', function ($request, $response) {
    try {
        switch ($request->server['request_uri']) {
        case "/":
            break;

        case "/curl":
            {
                $ch = curl_init();
                curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9501/");
                curl_setopt($ch, CURLOPT_TIMEOUT, 10);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_HEADER, 0);
                $output = curl_exec($ch);
                curl_close($ch);
                Assert::same($output, "ok");
            }

            {
                $ch = curl_init();
                curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9502/");
                curl_setopt($ch, CURLOPT_TIMEOUT, 10);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_HEADER, 0);
                $output = curl_exec($ch);
                curl_close($ch);
                Assert::same($output, "ok");
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
