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
    $ch = curl_init();
    curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9011/index.php");
    curl_setopt($ch, CURLOPT_TIMEOUT, 10);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
    curl_setopt($ch, CURLOPT_HEADER, 0);
    $output = curl_exec($ch);
    curl_close($ch);
    Assert::same($output, "ok");
}

{
    $ch = curl_init();
    curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9011/curl.test.php?single=1");
    curl_setopt($ch, CURLOPT_TIMEOUT, 10);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
    curl_setopt($ch, CURLOPT_POST, 1);
    curl_setopt($ch, CURLOPT_POSTFIELDS, ["foo" => "bar"]);
    curl_setopt($ch, CURLOPT_HTTPHEADER, ["X-FOO:BAR"]);
    $output = curl_exec($ch);
    curl_close($ch);
    Assert::same($output, "ok");
}

{
    $ch = curl_init();
    curl_setopt_array($ch, [
        CURLOPT_URL => "http://127.0.0.1:9011/curl.test.php?single=2",
        CURLOPT_TIMEOUT => 10,
        CURLOPT_RETURNTRANSFER => 1,
        CURLOPT_POST => 1,
        CURLOPT_POSTFIELDS => ["foo" => "bar"],
        CURLOPT_HTTPHEADER => ["X-FOO: BAR"],
    ]);
    $output = curl_exec($ch);
    curl_close($ch);
    Assert::same($output, "ok");
}

{
    $ch = curl_init();
    curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9011/not-exists.php?single=3");
    curl_setopt($ch, CURLOPT_TIMEOUT, 10);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
    curl_setopt($ch, CURLOPT_HEADER, 0);
    curl_exec($ch);
    curl_close($ch);
}

{
    $ch = curl_init();
    curl_setopt($ch, CURLOPT_URL, "http://127.0.0.1:9012/guzzle.php");
    curl_setopt($ch, CURLOPT_TIMEOUT, 10);
    curl_setopt($ch, CURLOPT_RETURNTRANSFER, 1);
    curl_setopt($ch, CURLOPT_HEADER, 0);
    $output = curl_exec($ch);
    curl_close($ch);
    Assert::same($output, "ok");
}

echo "ok";
