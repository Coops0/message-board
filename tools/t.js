const body = {
  version: "1.0.0",
  queries: [
    {
      Query: {
        Commands: [
          {
            SemanticQueryDataShapeCommand: {
              Query: {
                Version: 2,
                From: [
                  {
                    Name: "s",
                    Entity: "Sheet1",
                    Type: 0,
                  },
                ],
                Select: [
                  {
                    Column: {
                      Expression: {
                        SourceRef: {
                          Source: "s",
                        },
                      },
                      Property: "Name",
                    },
                    Name: "Sheet1.Name",
                  },
                  {
                    Column: {
                      Expression: {
                        SourceRef: {
                          Source: "s",
                        },
                      },
                      Property: "Approval Status",
                    },
                    Name: "Sheet1.Approval Status",
                  },
                  {
                    Column: {
                      Expression: {
                        SourceRef: {
                          Source: "s",
                        },
                      },
                      Property: "Usage Recommendations",
                    },
                    Name: "Sheet1.Usage Recommendations",
                  },
                  {
                    Column: {
                      Expression: {
                        SourceRef: {
                          Source: "s",
                        },
                      },
                      Property: "Privacy Policy",
                    },
                    Name: "Sheet1.Privacy Policy",
                  },
                  {
                    Column: {
                      Expression: {
                        SourceRef: {
                          Source: "s",
                        },
                      },
                      Property: "Terms of Service",
                    },
                    Name: "Sheet1.Terms of Service",
                  },
                  {
                    Aggregation: {
                      Expression: {
                        Column: {
                          Expression: {
                            SourceRef: {
                              Source: "s",
                            },
                          },
                          Property: "Privacy Policy",
                        },
                      },
                      Function: 3,
                    },
                    Name: "Min(Sheet1.Privacy Policy)",
                  },
                  {
                    Aggregation: {
                      Expression: {
                        Column: {
                          Expression: {
                            SourceRef: {
                              Source: "s",
                            },
                          },
                          Property: "Terms of Service",
                        },
                      },
                      Function: 3,
                    },
                    Name: "Min(Sheet1.Terms of Service)",
                  },
                ],
                Where: [
                  {
                    Condition: {
                      Not: {
                        Expression: {
                          In: {
                            Expressions: [
                              {
                                Column: {
                                  Expression: {
                                    SourceRef: {
                                      Source: "s",
                                    },
                                  },
                                  Property: "Approval Status",
                                },
                              },
                            ],
                            Values: [
                              [
                                {
                                  Literal: {
                                    Value: "''",
                                  },
                                },
                              ],
                            ],
                          },
                        },
                      },
                    },
                  },
                  {
                    Condition: {
                      Not: {
                        Expression: {
                          In: {
                            Expressions: [
                              {
                                Column: {
                                  Expression: {
                                    SourceRef: {
                                      Source: "s",
                                    },
                                  },
                                  Property: "Approval Status",
                                },
                              },
                            ],
                            Values: [
                              [
                                {
                                  Literal: {
                                    Value: "'Prohibited'",
                                  },
                                },
                              ],
                            ],
                          },
                        },
                      },
                    },
                  },
                  {
                    Condition: {
                      Not: {
                        Expression: {
                          In: {
                            Expressions: [
                              {
                                Column: {
                                  Expression: {
                                    SourceRef: {
                                      Source: "s",
                                    },
                                  },
                                  Property: "Approval Status",
                                },
                              },
                            ],
                            Values: [
                              [
                                {
                                  Literal: {
                                    Value: "''",
                                  },
                                },
                              ],
                            ],
                          },
                        },
                      },
                    },
                  },
                ],
                OrderBy: [
                  {
                    Direction: 1,
                    Expression: {
                      Column: {
                        Expression: {
                          SourceRef: {
                            Source: "s",
                          },
                        },
                        Property: "Name",
                      },
                    },
                  },
                ],
              },
              Binding: {
                Primary: {
                  Groupings: [
                    {
                      Projections: [0, 1, 2, 3, 4, 5, 6],
                    },
                  ],
                },
                // DataReduction: {
                //   // DataVolume: 3,
                //   Primary: {
                //     Window: {
                //       Count: 30000,
                //     },
                //   },
                //   // Primary: {
                //   //   Top: {
                //   //     Count: 30000,
                //   //   },
                //   // },
                // },
                // SuppressedJoinPredicates: [5, 6],
                Version: 1,
              },
              ExecutionMetricsKind: 1,
            },
          },
        ],
      },
      QueryId: "",
      ApplicationContext: {
        DatasetId: "863a52db-172f-47fa-8c3f-01ced8b05c18",
        Sources: [
          {
            ReportId: "6469fafd-c479-471f-94e0-c32446e50f18",
            VisualId: "3ae4b90679b861df9f89",
          },
        ],
      },
    },
  ],
  cancelQueries: [],
  modelId: 168870,
};

const fuckOffs = [
  "google.com",
  "cloudfront.net",
  "adobe.com",
  "amazon.com",
  "canva.com",
  "squarespace.com",
  "code.org",
];

const domainFromUri = (uri) => {
  let url;
  try {
    url = new URL(uri);
  } catch (e) {
    return null;
  }

  let { hostname } = url;
  if (hostname.startsWith("www.")) {
    hostname = hostname.slice(4);
  }

  if (fuckOffs.some((fo) => hostname.endsWith(fo))) {
    return null;
  }

  if (hostname.split(".").length > 2) {
    console.warn("possible subdomain detected", hostname);
  }

  return hostname;
};

async function b() {
  const response = await fetch(
    "https://wabi-us-east2-d-primary-api.analysis.windows.net/public/reports/querydata?synchronous=true",
    {
      headers: {
        accept: "application/json, text/plain, */*",
        "accept-language": "en-US,en;q=0.9",
        activityid: "d028049d-04f8-9a0d-3785-4a2a40dba52d",
        "cache-control": "no-cache",
        "content-type": "application/json;charset=UTF-8",
        pragma: "no-cache",
        requestid: "0966bfb1-a223-9495-67ba-316e6e9d117a",
        "sec-ch-ua": '"Chromium";v="131", "Not_A Brand";v="24"',
        "sec-ch-ua-mobile": "?0",
        "sec-ch-ua-platform": '"macOS"',
        "sec-fetch-dest": "empty",
        "sec-fetch-mode": "cors",
        "sec-fetch-site": "cross-site",
        "x-powerbi-resourcekey": "bf8e7d02-e88f-4bfd-a3cd-cbb8deacc7bf",
        Referer: "https://app.powerbi.com/",
        "Referrer-Policy": "strict-origin-when-cross-origin",
      },
      body: JSON.stringify(body),
      method: "POST",
    },
  ).then((r) => r.json());

  const dicts = Object.values(
    response.results[0].result.data.dsr.DS[0].ValueDicts,
  );

  const urls = new Set();
  for (let i = 0; i < dicts.length; i++) {
    const row = dicts[i];

    for (let j = 0; j < row.length; j++) {
      const entry = row[j];
      if (entry && entry.startsWith("http")) {
        const domain = domainFromUri(entry);
        if (domain) {
          urls.add(domain);
        }
      }
    }
  }

  console.log(urls, urls.size);
}

b();
