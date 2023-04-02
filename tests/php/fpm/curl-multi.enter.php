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

{
    curl_multi_request([]);
}

{
    $curl_callbacks = [
        [
            'curl' => (function () {
                $ch = curl_init();
                curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9011/curl.test.php?multi=1");
                curl_setopt($ch, CURLOPT_TIMEOUT, 10);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_POST, 1);
                curl_setopt($ch, CURLOPT_POSTFIELDS, ["foo" => "bar"]);
                curl_setopt($ch, CURLOPT_HTTPHEADER, ["X-FOO:BAR"]);
                return $ch;
            })(),
            'callback' => function ($output) {
                Assert::same($output, "ok");
            },
        ],
        [
            'curl' => (function () {
                $ch = curl_init();
                curl_setopt_array($ch, [
                    CURLOPT_URL => "http://127.0.0.1:9011/curl.test.php?multi=2",
                    CURLOPT_TIMEOUT => 10,
                    CURLOPT_RETURNTRANSFER => 1,
                    CURLOPT_POST => 1,
                    CURLOPT_POSTFIELDS => ["foo" => "bar"],
                    CURLOPT_HTTPHEADER => ["X-FOO: BAR"],
                ]);
                return $ch;
            })(),
            'callback' => function ($output) {
                Assert::same($output, "ok");
            },
        ],
        [
            'curl' => (function () {
                $ch = curl_init();
                curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9011/not-exists.php?multi=3");
                curl_setopt($ch, CURLOPT_TIMEOUT, 10);
                curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
                curl_setopt($ch, CURLOPT_HEADER, 0);
                return $ch;
            })(),
            'callback' => function ($output) {},
        ],
    ];
    curl_multi_request($curl_callbacks);
}

sleep(5);

echo "ok";

function curl_multi_request($curl_callbacks) {
    $mh = curl_multi_init();

    foreach ($curl_callbacks as $curl_callback) {
        curl_multi_add_handle($mh, $curl_callback['curl']);
    }

    do {
        $mrc = curl_multi_exec($mh, $active);
    } while ($mrc == CURLM_CALL_MULTI_PERFORM);

    while ($active && $mrc == CURLM_OK) {
        if (curl_multi_select($mh) == -1) {
            return;
        }
        do {
            $mrc = curl_multi_exec($mh, $active);
        } while ($mrc == CURLM_CALL_MULTI_PERFORM);
    }

    while ($info = curl_multi_info_read($mh)) {
        $content = curl_multi_getcontent($info['handle']);
        foreach ($curl_callbacks as $curl_callback) {
            if ($curl_callback['curl'] == $info['handle']) {
                call_user_func($curl_callback['callback'], $content);
                break;
            }
        }
    }

    foreach ($curl_callbacks as $curl_callback) {
        curl_multi_remove_handle($mh, $curl_callback['curl']);
    }

    curl_multi_close($mh);
}
